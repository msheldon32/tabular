use std::cmp;
use std::collections::HashSet;
use std::cell::RefCell;
use std::rc::Rc;

use crate::mode::Mode;
use crate::util::translate_references;

/// Number of rows per chunk for memory-efficient storage
pub const CHUNK_SIZE: usize = 1024;

/// Pure data structure for the table with chunked row storage
#[derive(Debug, Clone)]
pub struct Table {
    /// Rows stored in fixed-size chunks for memory efficiency
    chunks: Vec<Vec<Vec<String>>>,
    /// Total number of rows
    total_rows: usize,
    /// Number of columns
    col_count: usize,
    /// Cached column widths (max length of any cell in each column)
    col_widths: Vec<usize>,
    /// Whether col_widths needs full recompute
    col_widths_dirty: bool,
}

impl Table {
    /// Compute which chunk a row belongs to
    #[inline]
    fn chunk_idx(row: usize) -> usize {
        row / CHUNK_SIZE
    }

    /// Compute the index within a chunk
    #[inline]
    fn row_in_chunk(row: usize) -> usize {
        row % CHUNK_SIZE
    }

    /// Get a reference to the chunk containing a row
    #[inline]
    fn get_chunk(&self, row: usize) -> Option<&Vec<Vec<String>>> {
        self.chunks.get(Self::chunk_idx(row))
    }

    /// Get a mutable reference to the chunk containing a row
    #[inline]
    fn get_chunk_mut(&mut self, row: usize) -> Option<&mut Vec<Vec<String>>> {
        self.chunks.get_mut(Self::chunk_idx(row))
    }

    /// Get a mutable reference to a row
    pub fn get_row_mut(&mut self, idx: usize) -> Option<&mut Vec<String>> {
        if idx >= self.total_rows {
            return None;
        }
        let chunk = self.get_chunk_mut(idx)?;
        chunk.get_mut(Self::row_in_chunk(idx))
    }

    /// Swap two rows in the table
    pub fn swap_rows(&mut self, i: usize, j: usize) {
        if i == j || i >= self.total_rows || j >= self.total_rows {
            return;
        }

        let chunk_i = Self::chunk_idx(i);
        let chunk_j = Self::chunk_idx(j);
        let row_in_i = Self::row_in_chunk(i);
        let row_in_j = Self::row_in_chunk(j);

        if chunk_i == chunk_j {
            // Same chunk - simple swap
            self.chunks[chunk_i].swap(row_in_i, row_in_j);
        } else {
            // Different chunks - need to swap between chunks
            // Use a temporary to avoid borrow conflicts
            let row_i = std::mem::take(&mut self.chunks[chunk_i][row_in_i]);
            let row_j = std::mem::take(&mut self.chunks[chunk_j][row_in_j]);
            self.chunks[chunk_i][row_in_i] = row_j;
            self.chunks[chunk_j][row_in_j] = row_i;
        }
    }

    /// Clone all rows into a flat Vec (for undo/redo operations)
    pub fn clone_all_rows(&self) -> Vec<Vec<String>> {
        self.chunks.iter().flat_map(|chunk| chunk.iter().cloned()).collect()
    }

    /// Restore table from a flat Vec of rows
    pub fn restore_from_rows(&mut self, rows: Vec<Vec<String>>) {
        self.total_rows = rows.len();
        self.col_count = rows.first().map(|r| r.len()).unwrap_or(0);
        self.chunks = rows
            .chunks(CHUNK_SIZE)
            .map(|chunk| chunk.to_vec())
            .collect();
        self.mark_widths_dirty();
    }

    /// Iterator over all rows
    pub fn rows_iter(&self) -> impl Iterator<Item = &Vec<String>> {
        self.chunks.iter().flat_map(|chunk| chunk.iter())
    }

    pub fn new(cells: Vec<Vec<String>>) -> Self {
        let total_rows = cells.len();
        let col_count = cells.first().map(|r| r.len()).unwrap_or(0);

        let chunks: Vec<Vec<Vec<String>>> = cells
            .chunks(CHUNK_SIZE)
            .map(|chunk| chunk.to_vec())
            .collect();

        let mut table = Self {
            chunks,
            total_rows,
            col_count,
            col_widths: Vec::new(),
            col_widths_dirty: true,
        };
        table.recompute_col_widths();
        table
    }

    /// Create a table from pre-chunked data (avoids intermediate allocation)
    pub fn from_chunks(chunks: Vec<Vec<Vec<String>>>, col_count: usize) -> Self {
        let total_rows: usize = chunks.iter().map(|c| c.len()).sum();
        let mut table = Self {
            chunks,
            total_rows,
            col_count,
            col_widths: Vec::new(),
            col_widths_dirty: true,
        };
        table.recompute_col_widths();
        table
    }

    /// Create a table from pre-chunked data, deferring column width computation
    /// until first render. Use this for faster file loading.
    pub fn from_chunks_lazy(chunks: Vec<Vec<Vec<String>>>, col_count: usize) -> Self {
        let total_rows: usize = chunks.iter().map(|c| c.len()).sum();
        Self {
            chunks,
            total_rows,
            col_count,
            col_widths: Vec::new(),
            col_widths_dirty: true,
        }
    }

    /// Get cached column widths, recomputing if dirty
    pub fn col_widths(&mut self) -> &[usize] {
        if self.col_widths_dirty {
            self.recompute_col_widths();
        }
        &self.col_widths
    }

    /// Get column widths without mutable borrow (may be stale)
    pub fn col_widths_cached(&self) -> &[usize] {
        &self.col_widths
    }

    /// Force recompute of column widths
    pub fn recompute_col_widths(&mut self) {
        self.col_widths = (0..self.col_count)
            .map(|col| {
                self.rows_iter()
                    .filter_map(|row| row.get(col))
                    .map(|s| s.len())
                    .max()
                    .unwrap_or(3)
                    .max(3)
            })
            .collect();
        self.col_widths_dirty = false;
    }

    /// Mark column widths as needing recompute
    #[inline]
    fn mark_widths_dirty(&mut self) {
        self.col_widths_dirty = true;
    }

    /// Update width for a single column (when cell changes)
    #[inline]
    fn update_col_width(&mut self, col: usize, new_len: usize) {
        if col < self.col_widths.len() {
            self.col_widths[col] = self.col_widths[col].max(new_len).max(3);
        }
    }

    pub fn get_cell(&self, row: usize, col: usize) -> Option<&String> {
        self.get_chunk(row)?
            .get(Self::row_in_chunk(row))?
            .get(col)
    }

    pub fn set_cell(&mut self, row: usize, col: usize, value: String) {
        if let Some(chunk) = self.get_chunk_mut(row) {
            if let Some(r) = chunk.get_mut(Self::row_in_chunk(row)) {
                if let Some(cell) = r.get_mut(col) {
                    let new_len = value.len();
                    *cell = value;

                    // Update column width incrementally (only grows, never shrinks)
                    self.update_col_width(col, new_len);
                }
            }
        }
    }

    pub fn row_count(&self) -> usize {
        self.total_rows
    }

    pub fn col_count(&self) -> usize {
        self.col_count
    }

    pub fn insert_row_at(&mut self, idx: usize) {
        let new_row = vec![String::new(); self.col_count];
        self.insert_row_internal(idx, new_row);
    }

    /// Internal helper to insert a row and handle chunk rebalancing
    fn insert_row_internal(&mut self, idx: usize, row: Vec<String>) {
        if self.chunks.is_empty() {
            self.chunks.push(vec![row]);
            self.total_rows = 1;
            return;
        }

        let chunk_idx = Self::chunk_idx(idx.min(self.total_rows));
        let row_in_chunk = if idx >= self.total_rows {
            // Appending at end
            let last_chunk = self.chunks.len() - 1;
            self.chunks[last_chunk].len()
        } else {
            Self::row_in_chunk(idx)
        };

        // Insert into the appropriate chunk
        let actual_chunk = chunk_idx.min(self.chunks.len() - 1);
        let insert_pos = row_in_chunk.min(self.chunks[actual_chunk].len());
        self.chunks[actual_chunk].insert(insert_pos, row);
        self.total_rows += 1;

        // Rebalance chunks if needed (cascade overflow to next chunks)
        self.rebalance_chunks_after_insert(actual_chunk);
    }

    /// Rebalance chunks after an insert to maintain CHUNK_SIZE invariant
    fn rebalance_chunks_after_insert(&mut self, start_chunk: usize) {
        let mut chunk_idx = start_chunk;
        while chunk_idx < self.chunks.len() && self.chunks[chunk_idx].len() > CHUNK_SIZE {
            let overflow = self.chunks[chunk_idx].split_off(CHUNK_SIZE);
            if chunk_idx + 1 < self.chunks.len() {
                // Prepend overflow to next chunk
                let next_chunk = &mut self.chunks[chunk_idx + 1];
                for (i, row) in overflow.into_iter().enumerate() {
                    next_chunk.insert(i, row);
                }
            } else {
                // Create new chunk with overflow
                self.chunks.push(overflow);
            }
            chunk_idx += 1;
        }
    }

    pub fn delete_row_at(&mut self, idx: usize) -> Option<Vec<String>> {
        if self.total_rows <= 1 {
            // Clear the only row instead of deleting
            let row = self.get_row_cloned(0)?;
            if let Some(chunk) = self.chunks.first_mut() {
                if let Some(r) = chunk.first_mut() {
                    *r = vec![String::new(); self.col_count];
                }
            }
            self.mark_widths_dirty();
            return Some(row);
        }

        if idx >= self.total_rows {
            return None;
        }

        let chunk_idx = Self::chunk_idx(idx);
        let row_in_chunk = Self::row_in_chunk(idx);

        let removed = self.chunks[chunk_idx].remove(row_in_chunk);
        self.total_rows -= 1;

        // Rebalance: pull rows from subsequent chunks to maintain CHUNK_SIZE invariant
        self.rebalance_chunks_after_delete(chunk_idx);

        self.mark_widths_dirty();
        Some(removed)
    }

    /// Rebalance chunks after a delete to maintain CHUNK_SIZE invariant
    /// All chunks except the last should have exactly CHUNK_SIZE rows
    fn rebalance_chunks_after_delete(&mut self, start_chunk: usize) {
        let mut chunk_idx = start_chunk.min(self.chunks.len().saturating_sub(1));

        while chunk_idx < self.chunks.len() {
            // Remove empty chunks (except keep at least one chunk total)
            if self.chunks[chunk_idx].is_empty() && self.chunks.len() > 1 {
                self.chunks.remove(chunk_idx);
                // Don't increment - check the same index (now pointing to next chunk)
                // But also need to re-check previous chunk if it exists
                if chunk_idx > 0 {
                    chunk_idx -= 1;
                }
                continue;
            }

            // If this isn't the last chunk and it's under-filled, keep pulling from
            // subsequent chunks until full or no more chunks to pull from
            while chunk_idx + 1 < self.chunks.len() && self.chunks[chunk_idx].len() < CHUNK_SIZE {
                // If next chunk is empty, remove it and continue trying
                if self.chunks[chunk_idx + 1].is_empty() {
                    self.chunks.remove(chunk_idx + 1);
                    continue;
                }

                let needed = CHUNK_SIZE - self.chunks[chunk_idx].len();
                let available = self.chunks[chunk_idx + 1].len().min(needed);

                // Pull rows from beginning of next chunk
                let pulled: Vec<Vec<String>> = self.chunks[chunk_idx + 1].drain(0..available).collect();
                self.chunks[chunk_idx].extend(pulled);

                // If we emptied the next chunk, remove it
                if self.chunks[chunk_idx + 1].is_empty() {
                    self.chunks.remove(chunk_idx + 1);
                }
            }

            chunk_idx += 1;
        }
    }

    /// Delete multiple contiguous rows efficiently, handling cross-chunk boundaries
    /// Returns the deleted rows for undo/clipboard
    pub fn delete_rows_bulk(&mut self, start_idx: usize, count: usize) -> Vec<Vec<String>> {
        if count == 0 || start_idx >= self.total_rows {
            return Vec::new();
        }

        // Clamp count to available rows
        let actual_count = count.min(self.total_rows - start_idx);

        // Don't delete all rows - keep at least one (cleared)
        if actual_count >= self.total_rows {
            let deleted = self.clone_all_rows();
            // Clear the table to a single empty row
            self.chunks = vec![vec![vec![String::new(); self.col_count]]];
            self.total_rows = 1;
            self.mark_widths_dirty();
            return deleted;
        }

        let end_idx = start_idx + actual_count;
        let start_chunk = Self::chunk_idx(start_idx);
        let end_chunk = Self::chunk_idx(end_idx.saturating_sub(1));

        let mut deleted = Vec::with_capacity(actual_count);

        if start_chunk == end_chunk {
            // All deletions within a single chunk
            let row_start = Self::row_in_chunk(start_idx);
            let row_end = row_start + actual_count;
            deleted.extend(self.chunks[start_chunk].drain(row_start..row_end));
        } else {
            // Cross-chunk deletion: iterate forward to maintain correct row order
            // drain() doesn't affect chunk indices, just empties portions of each chunk

            // Handle start chunk (partial: from start_row to end of chunk)
            let start_row_in_chunk = Self::row_in_chunk(start_idx);
            deleted.extend(self.chunks[start_chunk].drain(start_row_in_chunk..));

            // Handle middle chunks (complete removal) - forward order preserves row order
            for chunk_idx in (start_chunk + 1)..end_chunk {
                deleted.extend(self.chunks[chunk_idx].drain(..));
            }

            // Handle end chunk (partial: from start to end_row)
            let end_row_in_chunk = Self::row_in_chunk(end_idx.saturating_sub(1)) + 1;
            deleted.extend(self.chunks[end_chunk].drain(0..end_row_in_chunk));
        }

        self.total_rows -= actual_count;
        self.rebalance_chunks_after_delete(start_chunk);
        self.mark_widths_dirty();

        deleted
    }

    /// Insert multiple empty rows at the specified index
    pub fn insert_rows_bulk(&mut self, idx: usize, count: usize) {
        if count == 0 {
            return;
        }
        let new_rows: Vec<Vec<String>> = (0..count)
            .map(|_| vec![String::new(); self.col_count])
            .collect();
        self.insert_rows_with_data_bulk(idx, new_rows);
    }

    /// Insert multiple rows with data at the specified index, handling cross-chunk boundaries
    pub fn insert_rows_with_data_bulk(&mut self, idx: usize, mut rows: Vec<Vec<String>>) {
        if rows.is_empty() {
            return;
        }

        let count = rows.len();

        // Ensure all rows have correct column count
        for row in &mut rows {
            row.resize(self.col_count, String::new());
        }

        // Update widths for new data
        for row in &rows {
            for (col, val) in row.iter().enumerate() {
                self.update_col_width(col, val.len());
            }
        }

        if self.chunks.is_empty() {
            // Table is empty, just create new chunks from the rows
            self.chunks = rows.chunks(CHUNK_SIZE).map(|c| c.to_vec()).collect();
            self.total_rows = count;
            return;
        }

        let insert_idx = idx.min(self.total_rows);
        let chunk_idx = Self::chunk_idx(insert_idx);
        let row_in_chunk = if insert_idx >= self.total_rows {
            // Appending at end
            self.chunks.last().map(|c| c.len()).unwrap_or(0)
        } else {
            Self::row_in_chunk(insert_idx)
        };

        let actual_chunk = chunk_idx.min(self.chunks.len().saturating_sub(1));

        // For large inserts, it's more efficient to rebuild from this point
        if count > CHUNK_SIZE {
            // Collect all rows from insert point onward
            let mut tail_rows: Vec<Vec<String>> = Vec::new();

            // Get rows after insert point in the current chunk
            if row_in_chunk < self.chunks[actual_chunk].len() {
                tail_rows.extend(self.chunks[actual_chunk].drain(row_in_chunk..));
            }

            // Get all rows from subsequent chunks
            for chunk in self.chunks.drain((actual_chunk + 1)..) {
                tail_rows.extend(chunk);
            }

            // Extend current chunk with new rows, then tail
            self.chunks[actual_chunk].extend(rows);
            self.chunks[actual_chunk].extend(tail_rows);

            // Rechunk from this point
            if self.chunks[actual_chunk].len() > CHUNK_SIZE {
                let overflow = self.chunks[actual_chunk].split_off(CHUNK_SIZE);
                let new_chunks: Vec<Vec<Vec<String>>> = overflow
                    .chunks(CHUNK_SIZE)
                    .map(|c| c.to_vec())
                    .collect();
                self.chunks.extend(new_chunks);
            }
        } else {
            // Small insert: insert directly and rebalance
            let insert_pos = row_in_chunk.min(self.chunks[actual_chunk].len());

            // Insert rows in reverse order at the same position
            for row in rows.into_iter().rev() {
                self.chunks[actual_chunk].insert(insert_pos, row);
            }

            self.rebalance_chunks_after_insert(actual_chunk);
        }

        self.total_rows += count;
    }

    /// Fill/paste multiple contiguous rows with data, expanding table if needed
    pub fn fill_rows_with_data_bulk(&mut self, start_idx: usize, rows: Vec<Vec<String>>) {
        if rows.is_empty() {
            return;
        }

        let count = rows.len();

        // Ensure table is large enough
        let needed_rows = start_idx + count;
        if needed_rows > self.total_rows {
            self.insert_rows_bulk(self.total_rows, needed_rows - self.total_rows);
        }

        self.mark_widths_dirty();

        // Fill rows, handling cross-chunk boundaries efficiently
        let start_chunk = Self::chunk_idx(start_idx);
        let end_chunk = Self::chunk_idx(start_idx + count - 1);

        let mut row_iter = rows.into_iter();

        for chunk_idx in start_chunk..=end_chunk {
            if chunk_idx >= self.chunks.len() {
                break;
            }

            let first_row_in_chunk = if chunk_idx == start_chunk {
                Self::row_in_chunk(start_idx)
            } else {
                0
            };

            let last_row_in_chunk = if chunk_idx == end_chunk {
                Self::row_in_chunk(start_idx + count - 1) + 1
            } else {
                self.chunks[chunk_idx].len()
            };

            for row_in_chunk in first_row_in_chunk..last_row_in_chunk {
                if let Some(mut new_row) = row_iter.next() {
                    new_row.resize(self.col_count, String::new());
                    if row_in_chunk < self.chunks[chunk_idx].len() {
                        self.chunks[chunk_idx][row_in_chunk] = new_row;
                    }
                }
            }
        }
    }

    /// Get multiple contiguous rows as cloned data (for clipboard/undo)
    pub fn get_rows_cloned(&self, start_idx: usize, count: usize) -> Vec<Vec<String>> {
        if start_idx >= self.total_rows || count == 0 {
            return Vec::new();
        }

        let actual_count = count.min(self.total_rows - start_idx);
        let mut result = Vec::with_capacity(actual_count);

        let start_chunk = Self::chunk_idx(start_idx);
        let end_chunk = Self::chunk_idx(start_idx + actual_count - 1);

        for chunk_idx in start_chunk..=end_chunk {
            if chunk_idx >= self.chunks.len() {
                break;
            }

            let first_row = if chunk_idx == start_chunk {
                Self::row_in_chunk(start_idx)
            } else {
                0
            };

            let last_row = if chunk_idx == end_chunk {
                Self::row_in_chunk(start_idx + actual_count - 1) + 1
            } else {
                self.chunks[chunk_idx].len()
            };

            for row_in_chunk in first_row..last_row {
                if row_in_chunk < self.chunks[chunk_idx].len() {
                    result.push(self.chunks[chunk_idx][row_in_chunk].clone());
                }
            }
        }

        result
    }

    pub fn insert_col_at(&mut self, idx: usize) {
        for chunk in &mut self.chunks {
            for row in chunk {
                if idx <= row.len() {
                    row.insert(idx, String::new());
                }
            }
        }
        self.col_count += 1;
        // Insert new column width (minimum width)
        if idx <= self.col_widths.len() {
            self.col_widths.insert(idx, 3);
        }
    }

    pub fn delete_col_at(&mut self, idx: usize) -> Option<Vec<String>> {
        if self.col_count <= 1 {
            let col: Vec<String> = self.rows_iter().map(|r| r[0].clone()).collect();
            for chunk in &mut self.chunks {
                for row in chunk {
                    row[0] = String::new();
                }
            }
            self.mark_widths_dirty();
            return Some(col);
        }

        if idx >= self.col_count {
            return None;
        }

        let col: Vec<String> = self.rows_iter().map(|r| r[idx].clone()).collect();
        for chunk in &mut self.chunks {
            for row in chunk {
                if idx < row.len() {
                    row.remove(idx);
                }
            }
        }
        self.col_count -= 1;
        // Remove the column width entry
        if idx < self.col_widths.len() {
            self.col_widths.remove(idx);
        }
        Some(col)
    }

    /// Get a reference to a row (no cloning)
    #[inline]
    pub fn get_row(&self, idx: usize) -> Option<&[String]> {
        if idx >= self.total_rows {
            return None;
        }
        self.get_chunk(idx)?
            .get(Self::row_in_chunk(idx))
            .map(|v| v.as_slice())
    }

    /// Get a cloned copy of a row (for transactions/clipboard)
    pub fn get_row_cloned(&self, idx: usize) -> Option<Vec<String>> {
        if idx >= self.total_rows {
            return None;
        }
        self.get_chunk(idx)?
            .get(Self::row_in_chunk(idx))
            .cloned()
    }

    /// Get a cloned copy of a column (for transactions/clipboard)
    pub fn get_col_cloned(&self, idx: usize) -> Option<Vec<String>> {
        if idx >= self.col_count {
            return None;
        }
        Some(self.rows_iter().map(|r| r[idx].clone()).collect())
    }

    /// Iterate over column values without cloning
    pub fn col_iter(&self, idx: usize) -> impl Iterator<Item = &String> {
        self.rows_iter().filter_map(move |r| r.get(idx))
    }

    pub fn get_span(&self, start_row: usize, end_row: usize, start_col: usize, end_col: usize) -> Option<Vec<Vec<String>>> {
        let mut out_vec = Vec::new();

        for row_iter in start_row..=end_row {
            let mut row = Vec::new();
            for col_iter in start_col..=end_col {
                // Return empty string for out-of-bounds cells
                let value = self.get_cell(row_iter, col_iter)
                    .cloned()
                    .unwrap_or_default();
                row.push(value);
            }
            out_vec.push(row);
        }

        Some(out_vec)
    }

    /// Ensure the table has at least the specified dimensions
    pub fn ensure_size(&mut self, rows: usize, cols: usize) {
        // Add rows if needed
        while self.total_rows < rows {
            self.insert_row_at(self.total_rows);
        }
        // Add columns if needed
        if cols > self.col_count {
            for chunk in &mut self.chunks {
                for row in chunk {
                    while row.len() < cols {
                        row.push(String::new());
                    }
                }
            }
            // Extend col_widths if we added columns
            while self.col_widths.len() < cols {
                self.col_widths.push(3);
            }
            self.col_count = cols;
        }
    }

    pub fn insert_row_with_data(&mut self, idx: usize, mut row: Vec<String>) {
        row.resize(self.col_count, String::new());
        // Update widths for new data
        for (col, val) in row.iter().enumerate() {
            self.update_col_width(col, val.len());
        }
        self.insert_row_internal(idx, row);
    }

    pub fn fill_row_with_data(&mut self, idx: usize, row: Vec<String>) {
        if row.len() != self.col_count {
            return;
        }
        // Mark dirty since we might shrink widths
        self.mark_widths_dirty();
        if let Some(target) = self.get_row_mut(idx) {
            *target = row;
        }
    }

    pub fn insert_col_with_data(&mut self, idx: usize, col: Vec<String>) {
        let max_width = col.iter().map(|s| s.len()).max().unwrap_or(0).max(3);
        let mut col_iter = col.into_iter();
        for chunk in &mut self.chunks {
            for row in chunk {
                if idx <= row.len() {
                    row.insert(idx, col_iter.next().unwrap_or_default());
                }
            }
        }
        self.col_count += 1;
        if idx <= self.col_widths.len() {
            self.col_widths.insert(idx, max_width);
        }
    }

    pub fn fill_col_with_data(&mut self, idx: usize, col: Vec<String>) {
        // Mark dirty since widths might shrink
        self.mark_widths_dirty();
        let mut col_iter = col.into_iter();
        for chunk in &mut self.chunks {
            for row in chunk {
                if idx < row.len() {
                    row[idx] = col_iter.next().unwrap_or_default();
                }
            }
        }
    }

    pub fn fill_span_with_data(&mut self, row_idx: usize, col_idx: usize, span: Vec<Vec<String>>) {
        for (dx, span_row) in span.iter().enumerate() {
            if row_idx + dx >= self.total_rows {
                self.insert_row_at(row_idx + dx);
            }
            for (dy, val) in span_row.iter().enumerate() {
                if col_idx + dy >= self.col_count {
                    self.insert_col_at(col_idx + dy);
                }
                self.set_cell(row_idx + dx, col_idx + dy, val.clone());
            }
        }
    }
}

impl Default for Table {
    fn default() -> Self {
        Table {
            chunks: vec![vec![vec![String::new()]]],
            total_rows: 1,
            col_count: 1,
            col_widths: vec![3],
            col_widths_dirty: false,
        }
    }
}

/// Represents whether a column/row should be sorted as numbers or text
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortType {
    Numeric,
    Text,
}

/// Sorting direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Compare two cell values for sorting
fn compare_cells(cell_a: &str, cell_b: &str, sort_type: SortType, direction: SortDirection) -> std::cmp::Ordering {
    let cmp = match sort_type {
        SortType::Numeric => {
            // Use parse_numeric to handle currency, percentages, etc.
            let num_a = crate::format::parse_numeric(cell_a).unwrap_or(f64::NAN);
            let num_b = crate::format::parse_numeric(cell_b).unwrap_or(f64::NAN);
            // Handle NaN: push non-parseable values to the end
            match (num_a.is_nan(), num_b.is_nan()) {
                (true, true) => cell_a.cmp(cell_b), // Both NaN, sort as text
                (true, false) => std::cmp::Ordering::Greater, // NaN goes last
                (false, true) => std::cmp::Ordering::Less,
                (false, false) => num_a.partial_cmp(&num_b).unwrap_or(std::cmp::Ordering::Equal),
            }
        }
        SortType::Text => cell_a.to_lowercase().cmp(&cell_b.to_lowercase()),
    };

    match direction {
        SortDirection::Ascending => cmp,
        SortDirection::Descending => cmp.reverse(),
    }
}

/// Maximum cells to sample for type detection
const TYPE_PROBE_SAMPLE_SIZE: usize = 20;

impl Table {
    /// Probe a column to determine if it's numeric or text
    /// Samples up to TYPE_PROBE_SAMPLE_SIZE non-empty cells for efficiency
    /// Recognizes formatted numbers (currency, percentages, etc.)
    pub fn probe_column_type(&self, col: usize, skip_header: bool) -> SortType {
        let start_row = if skip_header { 1 } else { 0 };
        let mut numeric_count = 0;
        let mut total_count = 0;

        for row_idx in start_row..self.row_count() {
            if total_count >= TYPE_PROBE_SAMPLE_SIZE {
                break;
            }
            if let Some(cell) = self.get_cell(row_idx, col) {
                let trimmed = cell.trim();
                if !trimmed.is_empty() {
                    total_count += 1;
                    if crate::format::parse_numeric(trimmed).is_some() {
                        numeric_count += 1;
                    }
                }
            }
        }

        // If more than half are numeric (or all are numeric), treat as numeric
        if total_count > 0 && numeric_count * 2 >= total_count {
            SortType::Numeric
        } else {
            SortType::Text
        }
    }

    /// Probe a row to determine if it's numeric or text
    /// Samples up to TYPE_PROBE_SAMPLE_SIZE non-empty cells for efficiency
    /// Recognizes formatted numbers (currency, percentages, etc.)
    pub fn probe_row_type(&self, row: usize, skip_first_col: bool) -> SortType {
        let start_col = if skip_first_col { 1 } else { 0 };
        let mut numeric_count = 0;
        let mut total_count = 0;

        if let Some(row_data) = self.get_row(row) {
            for col_idx in start_col..row_data.len() {
                if total_count >= TYPE_PROBE_SAMPLE_SIZE {
                    break;
                }
                let trimmed = row_data[col_idx].trim();
                if !trimmed.is_empty() {
                    total_count += 1;
                    if crate::format::parse_numeric(trimmed).is_some() {
                        numeric_count += 1;
                    }
                }
            }
        }

        if total_count > 0 && numeric_count * 2 >= total_count {
            SortType::Numeric
        } else {
            SortType::Text
        }
    }

    /// Sort rows by a specific column, returns the sorted indices
    /// skip_header: if true, row 0 is not included in sorting
    pub fn get_sorted_row_indices(
        &self,
        sort_col: usize,
        direction: SortDirection,
        skip_header: bool,
    ) -> Vec<usize> {
        let sort_type = self.probe_column_type(sort_col, skip_header);
        let start_row = if skip_header { 1 } else { 0 };

        let mut indices: Vec<usize> = (start_row..self.row_count()).collect();

        // Use get_cell for chunked access
        indices.sort_by(|&a, &b| {
            let cell_a = self.get_cell(a, sort_col).map(|s| s.trim()).unwrap_or("");
            let cell_b = self.get_cell(b, sort_col).map(|s| s.trim()).unwrap_or("");
            compare_cells(cell_a, cell_b, sort_type, direction)
        });

        // If we skipped header, prepend 0
        if skip_header {
            let mut result = vec![0];
            result.extend(indices);
            result
        } else {
            indices
        }
    }

    /// Sort columns by a specific row, returns the sorted column indices
    pub fn get_sorted_col_indices(
        &self,
        sort_row: usize,
        direction: SortDirection,
        skip_first_col: bool,
    ) -> Vec<usize> {
        let sort_type = self.probe_row_type(sort_row, skip_first_col);
        let start_col = if skip_first_col { 1 } else { 0 };

        let mut indices: Vec<usize> = (start_col..self.col_count()).collect();

        // Use get_row for chunked access
        if let Some(row) = self.get_row(sort_row) {
            indices.sort_by(|&a, &b| {
                let cell_a = row.get(a).map(|s| s.trim()).unwrap_or("");
                let cell_b = row.get(b).map(|s| s.trim()).unwrap_or("");
                compare_cells(cell_a, cell_b, sort_type, direction)
            });
        }

        if skip_first_col {
            let mut result = vec![0];
            result.extend(indices);
            result
        } else {
            indices
        }
    }

    /// Reorder rows according to the given indices
    /// Returns the old table state as Vec<Vec<String>> for undo
    pub fn reorder_rows(&mut self, new_order: &[usize]) -> Vec<Vec<String>> {
        let old_cells = self.clone_all_rows();
        let n = new_order.len().min(self.total_rows);

        // Build new row order
        let new_rows: Vec<Vec<String>> = new_order.iter()
            .take(n)
            .filter_map(|&idx| old_cells.get(idx).cloned())
            .collect();

        // Restore with new order
        self.restore_from_rows(new_rows);
        self.mark_widths_dirty();
        old_cells
    }

    /// Efficiently reorder rows in-place using the given indices
    /// Optimized for large tables - only clones once and works chunk-aware
    /// Returns the old table state for undo
    pub fn reorder_rows_bulk(&mut self, new_order: &[usize]) -> Vec<Vec<String>> {
        if new_order.len() != self.total_rows {
            return self.reorder_rows(new_order);
        }

        // Take ownership of chunks to avoid double-cloning
        let old_chunks = std::mem::take(&mut self.chunks);
        let old_total = self.total_rows;

        // Flatten into a single vec (we need random access by index)
        let mut flat_rows: Vec<Vec<String>> = old_chunks.into_iter().flatten().collect();

        // Build old_cells for undo by cloning (we need to return this)
        let old_cells = flat_rows.clone();

        // Reorder in-place using a permutation cycle approach for efficiency
        // This avoids creating a second full copy
        let mut new_rows = Vec::with_capacity(old_total);
        for &idx in new_order {
            if idx < flat_rows.len() {
                // Take the row, leaving an empty placeholder
                new_rows.push(std::mem::take(&mut flat_rows[idx]));
            }
        }

        // Rechunk the reordered rows
        self.chunks = new_rows
            .chunks(CHUNK_SIZE)
            .map(|chunk| chunk.to_vec())
            .collect();
        self.total_rows = self.chunks.iter().map(|c| c.len()).sum();
        self.mark_widths_dirty();

        old_cells
    }

    /// Sort rows by a column and apply the sort in-place
    /// Returns the old table state for undo
    pub fn sort_rows_by_column(
        &mut self,
        sort_col: usize,
        direction: SortDirection,
        skip_header: bool,
    ) -> Option<Vec<Vec<String>>> {
        let new_order = self.get_sorted_row_indices(sort_col, direction, skip_header);

        // Check if already sorted
        if new_order.iter().enumerate().all(|(i, &idx)| i == idx) {
            return None; // Already sorted
        }

        Some(self.reorder_rows_bulk(&new_order))
    }

    /// Sort columns by a row and apply the sort in-place
    /// Returns the old table state for undo
    pub fn sort_cols_by_row(
        &mut self,
        sort_row: usize,
        direction: SortDirection,
        skip_first_col: bool,
    ) -> Option<Vec<Vec<String>>> {
        let new_order = self.get_sorted_col_indices(sort_row, direction, skip_first_col);

        // Check if already sorted
        if new_order.iter().enumerate().all(|(i, &idx)| i == idx) {
            return None; // Already sorted
        }

        Some(self.reorder_cols(&new_order))
    }

    /// Apply a row permutation in-place (memory-efficient)
    /// permutation[i] = j means row i in new table comes from row j in old table
    pub fn apply_row_permutation(&mut self, permutation: &[usize]) {
        if permutation.len() != self.total_rows {
            return;
        }

        // Flatten chunks, apply permutation, rechunk
        let old_chunks = std::mem::take(&mut self.chunks);
        let mut flat_rows: Vec<Vec<String>> = old_chunks.into_iter().flatten().collect();

        // Build new order by taking rows according to permutation
        let mut new_rows = Vec::with_capacity(flat_rows.len());
        for &src_idx in permutation {
            if src_idx < flat_rows.len() {
                new_rows.push(std::mem::take(&mut flat_rows[src_idx]));
            }
        }

        // Rechunk
        self.chunks = new_rows
            .chunks(CHUNK_SIZE)
            .map(|chunk| chunk.to_vec())
            .collect();
        self.total_rows = self.chunks.iter().map(|c| c.len()).sum();
        self.mark_widths_dirty();
    }

    /// Apply a column permutation in-place
    /// permutation[i] = j means column i in new table comes from column j in old table
    pub fn apply_col_permutation(&mut self, permutation: &[usize]) {
        if permutation.len() != self.col_count {
            return;
        }

        for chunk in &mut self.chunks {
            for row in chunk {
                let old_row = row.clone();
                for (new_col, &src_col) in permutation.iter().enumerate() {
                    if new_col < row.len() && src_col < old_row.len() {
                        row[new_col] = old_row[src_col].clone();
                    }
                }
            }
        }

        // Reorder column widths to match
        let old_widths = self.col_widths.clone();
        for (new_col, &src_col) in permutation.iter().enumerate() {
            if new_col < self.col_widths.len() && src_col < old_widths.len() {
                self.col_widths[new_col] = old_widths[src_col];
            }
        }
    }

    /// Get the permutation needed to sort rows by a column
    /// Returns None if already sorted
    pub fn get_sort_permutation(
        &self,
        sort_col: usize,
        direction: SortDirection,
        skip_header: bool,
    ) -> Option<Vec<usize>> {
        let new_order = self.get_sorted_row_indices(sort_col, direction, skip_header);

        // Check if already sorted
        if new_order.iter().enumerate().all(|(i, &idx)| i == idx) {
            return None;
        }

        Some(new_order)
    }

    /// Get the permutation needed to sort columns by a row
    /// Returns None if already sorted
    pub fn get_col_sort_permutation(
        &self,
        sort_row: usize,
        direction: SortDirection,
        skip_first_col: bool,
    ) -> Option<Vec<usize>> {
        let new_order = self.get_sorted_col_indices(sort_row, direction, skip_first_col);

        // Check if already sorted
        if new_order.iter().enumerate().all(|(i, &idx)| i == idx) {
            return None;
        }

        Some(new_order)
    }

    /// Reorder columns according to the given indices
    /// Returns the old table state as Vec<Vec<String>> for undo
    pub fn reorder_cols(&mut self, new_order: &[usize]) -> Vec<Vec<String>> {
        let old_cells = self.clone_all_rows();
        let n = new_order.len();

        for chunk in &mut self.chunks {
            for row in chunk {
                if row.len() < n {
                    continue;
                }

                // Use a temporary buffer for this row
                let old_row: Vec<String> = new_order.iter()
                    .filter_map(|&idx| row.get(idx).cloned())
                    .collect();

                for (i, val) in old_row.into_iter().enumerate() {
                    if i < row.len() {
                        row[i] = val;
                    }
                }
            }
        }

        // Reorder column widths to match
        if self.col_widths.len() >= n {
            let old_widths = self.col_widths.clone();
            for (i, &idx) in new_order.iter().enumerate() {
                if i < self.col_widths.len() && idx < old_widths.len() {
                    self.col_widths[i] = old_widths[idx];
                }
            }
        }

        old_cells
    }
}

/// View state for the table (cursor, viewport, cached widths)
#[derive(Debug, Clone)]
pub struct TableView {
    // Cursor position
    pub cursor_row: usize,
    pub cursor_col: usize,

    // Support position (for visual mode)
    pub support_row: usize,
    pub support_col: usize,

    // Viewport offset (top-left visible cell)
    pub viewport_row: usize,
    pub viewport_col: usize,

    // Visible area size (set during render)
    pub visible_rows: usize,
    pub visible_cols: usize,

    // Cached column widths
    pub col_widths: Vec<usize>,
}

impl TableView {
    pub fn new() -> Self {
        Self {
            cursor_row: 0,
            cursor_col: 0,
            viewport_row: 0,
            viewport_col: 0,
            visible_rows: 20,
            visible_cols: 10,
            support_row: 0,
            support_col: 0,
            col_widths: Vec::new(),
        }
    }

    /// Update cached column widths based on table content
    /// Now delegates to Table's cached widths
    pub fn update_col_widths(&mut self, table: &Table) {
        self.col_widths = table.col_widths_cached().to_vec();
    }

    /// Sync column widths from a mutable table (forces recompute if dirty)
    pub fn sync_col_widths(&mut self, table: &mut Table) {
        self.col_widths = table.col_widths().to_vec();
    }

    pub fn is_selected(&mut self, row_idx: usize, col_idx: usize, mode: Mode) -> bool {
        let mut row_valid = true;
        let mut col_valid = true;
        if mode != Mode::VisualCol {
            row_valid = cmp::min(self.cursor_row, self.support_row) <= row_idx;
            row_valid = row_valid && row_idx <= cmp::max(self.cursor_row, self.support_row);
        }

        if mode != Mode::VisualRow {
            col_valid = cmp::min(self.cursor_col, self.support_col) <= col_idx;
            col_valid = col_valid && col_idx <= cmp::max(self.cursor_col, self.support_col);
        }

        return row_valid && col_valid;
    }

    pub fn set_support(&mut self) {
        self.support_row = self.cursor_row;
        self.support_col = self.cursor_col;
    }

    /// Get the bounds of the current selection (start_row, end_row, start_col, end_col)
    pub fn get_selection_bounds(&self) -> (usize, usize, usize, usize) {
        (
            cmp::min(self.cursor_row, self.support_row),
            cmp::max(self.cursor_row, self.support_row),
            cmp::min(self.cursor_col, self.support_col),
            cmp::max(self.cursor_col, self.support_col),
        )
    }

    pub fn expand_column(&mut self, length: usize) {
        self.col_widths[self.cursor_col] = cmp::max(self.col_widths[self.cursor_col], length);
    }

    /// Ensure cursor is within table bounds
    pub fn clamp_cursor(&mut self, table: &Table) {
        if table.row_count() > 0 {
            self.cursor_row = self.cursor_row.min(table.row_count() - 1);
        }
        if table.col_count() > 0 {
            self.cursor_col = self.cursor_col.min(table.col_count() - 1);
        }
    }

    /// Ensure viewport contains the cursor
    pub fn scroll_to_cursor(&mut self) {
        // Vertical scrolling
        if self.cursor_row < self.viewport_row {
            self.viewport_row = self.cursor_row;
        } else if self.cursor_row >= self.viewport_row + self.visible_rows {
            self.viewport_row = self.cursor_row.saturating_sub(self.visible_rows - 1);
        }

        // Horizontal scrolling
        if self.cursor_col < self.viewport_col {
            self.viewport_col = self.cursor_col;
        } else if self.cursor_col >= self.viewport_col + self.visible_cols {
            self.viewport_col = self.cursor_col.saturating_sub(self.visible_cols - 1);
        }
    }

    // Navigation methods
    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            self.scroll_to_cursor();
        }
    }

    pub fn move_right(&mut self, table: &Table) {
        if self.cursor_col + 1 < table.col_count() {
            self.cursor_col += 1;
            self.scroll_to_cursor();
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.scroll_to_cursor();
        }
    }

    pub fn move_down(&mut self, table: &Table) {
        if self.cursor_row + 1 < table.row_count() {
            self.cursor_row += 1;
            self.scroll_to_cursor();
        }
    }

    pub fn move_to_top(&mut self) {
        self.cursor_row = 0;
        self.scroll_to_cursor();
    }

    pub fn move_to_bottom(&mut self, table: &Table) {
        if table.row_count() > 0 {
            self.cursor_row = table.row_count() - 1;
            self.scroll_to_cursor();
        }
    }

    pub fn move_to_first_col(&mut self) {
        self.cursor_col = 0;
        self.scroll_to_cursor();
    }

    pub fn move_to_last_col(&mut self, table: &Table) {
        if table.col_count() > 0 {
            self.cursor_col = table.col_count() - 1;
            self.scroll_to_cursor();
        }
    }

    pub fn page_down(&mut self, table: &Table) {
        let jump = self.visible_rows.saturating_sub(1).max(1);
        self.cursor_row = (self.cursor_row + jump).min(table.row_count().saturating_sub(1));
        self.scroll_to_cursor();
    }

    pub fn page_up(&mut self) {
        let jump = self.visible_rows.saturating_sub(1).max(1);
        self.cursor_row = self.cursor_row.saturating_sub(jump);
        self.scroll_to_cursor();
    }

    pub fn half_page_down(&mut self, table: &Table) {
        let jump = self.visible_rows / 2;
        self.cursor_row = (self.cursor_row + jump).min(table.row_count().saturating_sub(1));
        self.scroll_to_cursor();
    }

    pub fn half_page_up(&mut self) {
        let jump = self.visible_rows / 2;
        self.cursor_row = self.cursor_row.saturating_sub(jump);
        self.scroll_to_cursor();
    }

    // Movement with count
    pub fn move_left_n(&mut self, n: usize) {
        self.cursor_col = self.cursor_col.saturating_sub(n);
        self.scroll_to_cursor();
    }

    pub fn move_right_n(&mut self, n: usize, table: &Table) {
        self.cursor_col = (self.cursor_col + n).min(table.col_count().saturating_sub(1));
        self.scroll_to_cursor();
    }

    pub fn move_up_n(&mut self, n: usize) {
        self.cursor_row = self.cursor_row.saturating_sub(n);
        self.scroll_to_cursor();
    }

    pub fn move_down_n(&mut self, n: usize, table: &Table) {
        self.cursor_row = (self.cursor_row + n).min(table.row_count().saturating_sub(1));
        self.scroll_to_cursor();
    }

    // Jump navigation (Ctrl+Arrow behavior like Excel)
    // If in occupied cell: jump to last occupied cell before empty/edge
    // If in empty cell: jump to first occupied cell in direction

    fn is_cell_occupied(table: &Table, row: usize, col: usize) -> bool {
        table.get_cell(row, col)
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }

    pub fn jump_left(&mut self, table: &Table) {
        if self.cursor_col == 0 {
            return;
        }

        let mut target = self.cursor_col - 1;

        let is_occupied = Self::is_cell_occupied(table, self.cursor_row, target);

        while target > 0 && (Self::is_cell_occupied(table, self.cursor_row, target-1) == is_occupied) {
            target -= 1;
        }

        if target > 0 && !is_occupied {
            target -= 1;
        }

        self.cursor_col = target;

        self.scroll_to_cursor();
    }

    pub fn jump_right(&mut self, table: &Table) {
        let max_col = table.col_count().saturating_sub(1);
        if self.cursor_col >= max_col {
            return;
        }

        let mut target = self.cursor_col + 1;

        let is_occupied = Self::is_cell_occupied(table, self.cursor_row, target);

        while target < max_col && (Self::is_cell_occupied(table, self.cursor_row, target+1) == is_occupied) {
            target += 1;
        }

        if target < max_col && !is_occupied {
            target += 1;
        }

        self.cursor_col = target;

        self.scroll_to_cursor();
    }

    pub fn jump_up(&mut self, table: &Table) {
        if self.cursor_row == 0 {
            return;
        }

        let mut target = self.cursor_row - 1;

        let is_occupied = Self::is_cell_occupied(table, target, self.cursor_col);

        while target > 0 && (Self::is_cell_occupied(table, target-1, self.cursor_col) == is_occupied) {
            target -= 1;
        }

        if target > 0 && !is_occupied {
            target -= 1;
        }

        self.cursor_row = target;

        self.scroll_to_cursor();
    }

    pub fn jump_down(&mut self, table: &Table) {
        let max_row = table.row_count().saturating_sub(1);
        if self.cursor_row >= max_row {
            return;
        }

        let mut target = self.cursor_row + 1;

        let is_occupied = Self::is_cell_occupied(table, target, self.cursor_col);

        while target < max_row && (Self::is_cell_occupied(table, target+1, self.cursor_col) == is_occupied) {
            target += 1;
        }

        if target < max_row && !is_occupied {
            target += 1;
        }

        self.cursor_row = target;

        self.scroll_to_cursor();
    }

    pub fn goto_row(&mut self, row: usize, table: &Table) {
        self.cursor_row = row.min(table.row_count().saturating_sub(1));
        self.scroll_to_cursor();
    }

    /// Get current cell content
    pub fn current_cell<'a>(&self, table: &'a Table) -> &'a String {
        table.get_cell(self.cursor_row, self.cursor_col)
            .expect("Cursor should be within bounds")
    }

    /// Get mutable reference to current cell
    pub fn current_cell_mut<'a>(&self, table: &'a mut Table) -> &'a mut String {
        table.get_row_mut(self.cursor_row)
            .and_then(|r| r.get_mut(self.cursor_col))
            .expect("Cursor should be within bounds")
    }

    // Row operations that update cursor
    pub fn insert_row_below(&mut self, table: &mut Table) {
        table.insert_row_at(self.cursor_row + 1);
        self.cursor_row += 1;
        self.scroll_to_cursor();
    }

    pub fn insert_row_above(&mut self, table: &mut Table) {
        table.insert_row_at(self.cursor_row);
        self.scroll_to_cursor();
    }

    pub fn delete_row(&mut self, table: &mut Table) -> Option<Vec<String>> {
        let row = table.delete_row_at(self.cursor_row);
        self.clamp_cursor(table);
        self.scroll_to_cursor();
        row
    }

    /// Delete multiple selected rows (for VisualRow mode), returns deleted rows
    pub fn delete_rows_bulk(&mut self, table: &mut Table) -> Vec<Vec<String>> {
        let (start_row, end_row, _, _) = self.get_selection_bounds();
        let count = end_row - start_row + 1;
        let deleted = table.delete_rows_bulk(start_row, count);
        self.cursor_row = start_row;
        self.support_row = start_row;
        self.clamp_cursor(table);
        self.scroll_to_cursor();
        deleted
    }

    pub fn yank_row(&self, table: &Table) -> Option<Vec<String>> {
        table.get_row_cloned(self.cursor_row)
    }

    /// Yank multiple selected rows (for VisualRow mode)
    pub fn yank_rows_bulk(&self, table: &Table) -> Vec<Vec<String>> {
        let (start_row, end_row, _, _) = self.get_selection_bounds();
        let count = end_row - start_row + 1;
        table.get_rows_cloned(start_row, count)
    }

    pub fn paste_row(&mut self, table: &mut Table, row: Vec<String>) {
        table.fill_row_with_data(self.cursor_row, row);
    }

    /// Paste multiple rows starting at cursor, overwriting existing rows
    pub fn paste_rows_bulk(&mut self, table: &mut Table, rows: Vec<Vec<String>>) {
        table.fill_rows_with_data_bulk(self.cursor_row, rows);
    }

    /// Insert multiple rows below current selection with data (e.g., after paste)
    pub fn insert_rows_below_bulk(&mut self, table: &mut Table, rows: Vec<Vec<String>>) {
        let count = rows.len();
        let insert_at = self.cursor_row + 1;
        table.insert_rows_with_data_bulk(insert_at, rows);
        self.cursor_row = insert_at;
        self.support_row = insert_at + count - 1;
        self.scroll_to_cursor();
    }

    /// Insert multiple empty rows below cursor
    pub fn insert_rows_below_empty(&mut self, table: &mut Table, count: usize) {
        let insert_at = self.cursor_row + 1;
        table.insert_rows_bulk(insert_at, count);
        self.cursor_row = insert_at;
        self.support_row = insert_at + count - 1;
        self.scroll_to_cursor();
    }

    /// Insert multiple rows above current selection with data
    pub fn insert_rows_above_bulk(&mut self, table: &mut Table, rows: Vec<Vec<String>>) {
        let count = rows.len();
        table.insert_rows_with_data_bulk(self.cursor_row, rows);
        self.support_row = self.cursor_row + count - 1;
        self.scroll_to_cursor();
    }

    // Column operations that update cursor
    pub fn insert_col_after(&mut self, table: &mut Table) {
        table.insert_col_at(self.cursor_col + 1);
        self.update_col_widths(table);
    }

    pub fn delete_col(&mut self, table: &mut Table) -> Option<Vec<String>> {
        let col = table.delete_col_at(self.cursor_col);
        self.clamp_cursor(table);
        self.update_col_widths(table);
        self.scroll_to_cursor();
        col
    }

    pub fn yank_col(&self, table: &Table) -> Option<Vec<String>> {
        table.get_col_cloned(self.cursor_col)
    }

    pub fn paste_col(&mut self, table: &mut Table, col: Vec<String>) {
        table.fill_col_with_data(self.cursor_col, col);
        self.update_col_widths(table);
    }

    pub fn yank_span(&self, table: &Table) -> Option<Vec<Vec<String>>> {
        let (start_row, end_row, start_col, end_col) = self.get_selection_bounds();
        table.get_span(start_row, end_row, start_col, end_col)
    }

    pub fn paste_span(&mut self, table: &mut Table, span: Vec<Vec<String>>) {
        table.fill_span_with_data(self.cursor_row, self.cursor_col, span);
    }

    pub fn clear_span(&mut self, table: &mut Table) {
        let (start_row, end_row, start_col, end_col) = self.get_selection_bounds();
        for row_idx in start_row..=end_row {
            for col_idx in start_col..=end_col {
                table.set_cell(row_idx, col_idx, String::new());
            }
        }
    }

    pub fn clear_row_span(&mut self, table: &mut Table) {
        let (start_row, end_row, _, _) = self.get_selection_bounds();
        let col_count = table.col_count();
        for row_idx in start_row..=end_row {
            for col_idx in 0..col_count {
                table.set_cell(row_idx, col_idx, String::new());
            }
        }
    }

    pub fn clear_col_span(&mut self, table: &mut Table) {
        let (_, _, start_col, end_col) = self.get_selection_bounds();
        let row_count = table.row_count();
        for row_idx in 0..row_count {
            for col_idx in start_col..=end_col {
                table.set_cell(row_idx, col_idx, String::new());
            }
        }
    }

    pub fn drag_down(&mut self, table: &mut Table, whole_row: bool) {
        let (start_row, end_row, sel_start_col, sel_end_col) = self.get_selection_bounds();
        let (start_col, end_col) = if whole_row {
            (0, table.col_count() - 1)
        } else {
            (sel_start_col, sel_end_col)
        };

        for row_idx in start_row+1..=end_row {
            for col_idx in start_col..=end_col {
                let source = table.get_cell(start_row, col_idx).cloned().unwrap_or_default();
                let new_val = translate_references(&source, (row_idx - start_row) as isize, 0isize);
                table.set_cell(row_idx, col_idx, new_val);
            }
        }
    }

    pub fn drag_up(&mut self, table: &mut Table, whole_row: bool) {
        let (start_row, end_row, sel_start_col, sel_end_col) = self.get_selection_bounds();
        let (start_col, end_col) = if whole_row {
            (0, table.col_count() - 1)
        } else {
            (sel_start_col, sel_end_col)
        };

        for row_idx in start_row..end_row {
            let offset = row_idx as isize - end_row as isize;
            for col_idx in start_col..=end_col {
                let source = table.get_cell(end_row, col_idx).cloned().unwrap_or_default();
                let new_val = translate_references(&source, offset, 0isize);
                table.set_cell(row_idx, col_idx, new_val);
            }
        }
    }

    pub fn drag_right(&mut self, table: &mut Table, whole_col: bool) {
        let (sel_start_row, sel_end_row, start_col, end_col) = self.get_selection_bounds();
        let (start_row, end_row) = if whole_col {
            (0, table.row_count() - 1)
        } else {
            (sel_start_row, sel_end_row)
        };

        for row_idx in start_row..=end_row {
            for col_idx in start_col+1..=end_col {
                let source = table.get_cell(row_idx, start_col).cloned().unwrap_or_default();
                let new_val = translate_references(&source, 0isize, (col_idx - start_col) as isize);
                table.set_cell(row_idx, col_idx, new_val);
            }
        }
    }

    pub fn drag_left(&mut self, table: &mut Table, whole_col: bool) {
        let (sel_start_row, sel_end_row, start_col, end_col) = self.get_selection_bounds();
        let (start_row, end_row) = if whole_col {
            (0, table.row_count() - 1)
        } else {
            (sel_start_row, sel_end_row)
        };

        for row_idx in start_row..=end_row {
            for col_idx in start_col..end_col {
                let offset = col_idx as isize - end_col as isize;
                let source = table.get_cell(row_idx, end_col).cloned().unwrap_or_default();
                let new_val = translate_references(&source, 0isize, offset);
                table.set_cell(row_idx, col_idx, new_val);
            }
        }
    }
}

impl Default for TableView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_table(data: Vec<Vec<&str>>) -> Table {
        Table::new(
            data.into_iter()
                .map(|row| row.into_iter().map(|s| s.to_string()).collect())
                .collect()
        )
    }

    /// Helper to get a row as Vec<String> for assertion comparisons
    fn row(table: &Table, idx: usize) -> Vec<String> {
        table.get_row(idx).unwrap().to_vec()
    }

    /// Helper to get a cell value as &str for assertion comparisons
    fn cell(table: &Table, r: usize, c: usize) -> String {
        table.get_cell(r, c).unwrap().clone()
    }

    // === Table basic operations ===

    #[test]
    fn test_table_new() {
        let table = Table::new(vec![vec!["".to_string()]]);
        assert_eq!(table.row_count(), 1);
        assert_eq!(table.col_count(), 1);
        assert_eq!(cell(&table, 0, 0), "");
    }

    #[test]
    fn test_table_get_cell() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        assert_eq!(table.get_cell(0, 0), Some(&"a".to_string()));
        assert_eq!(table.get_cell(0, 1), Some(&"b".to_string()));
        assert_eq!(table.get_cell(1, 0), Some(&"c".to_string()));
        assert_eq!(table.get_cell(1, 1), Some(&"d".to_string()));
        assert_eq!(table.get_cell(2, 0), None);
        assert_eq!(table.get_cell(0, 2), None);
    }

    #[test]
    fn test_table_set_cell() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.set_cell(0, 1, "x".to_string());
        assert_eq!(cell(&table, 0, 1), "x");

        // Out of bounds should not panic
        table.set_cell(10, 10, "y".to_string());
    }

    #[test]
    fn test_table_row_col_count() {
        let table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
        ]);

        assert_eq!(table.row_count(), 2);
        assert_eq!(table.col_count(), 3);
    }

    // === Row operations ===

    #[test]
    fn test_insert_row_at_beginning() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_row_at(0);

        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 0), vec!["", ""]);
        assert_eq!(row(&table, 1), vec!["a", "b"]);
    }

    #[test]
    fn test_insert_row_at_middle() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_row_at(1);

        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 0), vec!["a", "b"]);
        assert_eq!(row(&table, 1), vec!["", ""]);
        assert_eq!(row(&table, 2), vec!["c", "d"]);
    }

    #[test]
    fn test_insert_row_at_end() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_row_at(2);

        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 2), vec!["", ""]);
    }

    #[test]
    fn test_delete_row_at() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
            vec!["e", "f"],
        ]);

        let deleted = table.delete_row_at(1);

        assert_eq!(deleted, Some(vec!["c".to_string(), "d".to_string()]));
        assert_eq!(table.row_count(), 2);
        assert_eq!(row(&table, 0), vec!["a", "b"]);
        assert_eq!(row(&table, 1), vec!["e", "f"]);
    }

    #[test]
    fn test_delete_last_row_clears_instead() {
        let mut table = make_table(vec![
            vec!["a", "b"],
        ]);

        let deleted = table.delete_row_at(0);

        assert_eq!(deleted, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(table.row_count(), 1);
        assert_eq!(row(&table, 0), vec!["", ""]);
    }

    #[test]
    fn test_get_row() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        assert_eq!(table.get_row(0).map(|r| r.to_vec()), Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(table.get_row(1).map(|r| r.to_vec()), Some(vec!["c".to_string(), "d".to_string()]));
        assert_eq!(table.get_row(2), None);
    }

    // === Column operations ===

    #[test]
    fn test_insert_col_at_beginning() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_col_at(0);

        assert_eq!(table.col_count(), 3);
        assert_eq!(row(&table, 0), vec!["", "a", "b"]);
        assert_eq!(row(&table, 1), vec!["", "c", "d"]);
    }

    #[test]
    fn test_insert_col_at_middle() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_col_at(1);

        assert_eq!(table.col_count(), 3);
        assert_eq!(row(&table, 0), vec!["a", "", "b"]);
        assert_eq!(row(&table, 1), vec!["c", "", "d"]);
    }

    #[test]
    fn test_delete_col_at() {
        let mut table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
        ]);

        let deleted = table.delete_col_at(1);

        assert_eq!(deleted, Some(vec!["b".to_string(), "e".to_string()]));
        assert_eq!(table.col_count(), 2);
        assert_eq!(row(&table, 0), vec!["a", "c"]);
        assert_eq!(row(&table, 1), vec!["d", "f"]);
    }

    #[test]
    fn test_delete_last_col_clears_instead() {
        let mut table = make_table(vec![
            vec!["a"],
            vec!["b"],
        ]);

        let deleted = table.delete_col_at(0);

        assert_eq!(deleted, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(table.col_count(), 1);
        assert_eq!(row(&table, 0), vec![""]);
        assert_eq!(row(&table, 1), vec![""]);
    }

    #[test]
    fn test_get_col() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        assert_eq!(table.get_col_cloned(0), Some(vec!["a".to_string(), "c".to_string()]));
        assert_eq!(table.get_col_cloned(1), Some(vec!["b".to_string(), "d".to_string()]));
        assert_eq!(table.get_col_cloned(2), None);
    }

    // === Span operations ===

    #[test]
    fn test_get_span() {
        let table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["g", "h", "i"],
        ]);

        let span = table.get_span(0, 1, 0, 1).unwrap();
        assert_eq!(span, vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["d".to_string(), "e".to_string()],
        ]);
    }

    #[test]
    fn test_get_span_single_cell() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        let span = table.get_span(0, 0, 0, 0).unwrap();
        assert_eq!(span, vec![vec!["a".to_string()]]);
    }

    #[test]
    fn test_get_span_full_table() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        let span = table.get_span(0, 1, 0, 1).unwrap();
        assert_eq!(span, vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["c".to_string(), "d".to_string()],
        ]);
    }

    #[test]
    fn test_get_span_out_of_bounds() {
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        // Request span that extends beyond table bounds
        let span = table.get_span(0, 3, 0, 3).unwrap();
        assert_eq!(span.len(), 4); // 4 rows requested
        assert_eq!(span[0].len(), 4); // 4 cols requested
        // Valid cells have values, out of bounds are empty
        assert_eq!(span[0][0], "a");
        assert_eq!(span[0][1], "b");
        assert_eq!(span[0][2], ""); // out of bounds
        assert_eq!(span[2][0], ""); // out of bounds
    }

    #[test]
    fn test_ensure_size() {
        let mut table = make_table(vec![
            vec!["a", "b"],
        ]);

        assert_eq!(table.row_count(), 1);
        assert_eq!(table.col_count(), 2);

        table.ensure_size(3, 4);

        assert_eq!(table.row_count(), 3);
        assert_eq!(table.col_count(), 4);
        assert_eq!(cell(&table, 0, 0), "a");
        assert_eq!(cell(&table, 0, 3), "");
        assert_eq!(cell(&table, 2, 0), "");
    }

    // === Insert with data ===

    #[test]
    fn test_insert_row_with_data() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_row_with_data(1, vec!["x".to_string(), "y".to_string()]);

        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 1), vec!["x", "y"]);
    }

    #[test]
    fn test_insert_row_with_data_pads_short_row() {
        let mut table = make_table(vec![
            vec!["a", "b", "c"],
        ]);

        table.insert_row_with_data(1, vec!["x".to_string()]);

        assert_eq!(row(&table, 1), vec!["x", "", ""]);
    }

    #[test]
    fn test_insert_col_with_data() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.insert_col_with_data(1, vec!["x".to_string(), "y".to_string()]);

        assert_eq!(row(&table, 0), vec!["a", "x", "b"]);
        assert_eq!(row(&table, 1), vec!["c", "y", "d"]);
    }

    // === Fill operations ===

    #[test]
    fn test_fill_row_with_data() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.fill_row_with_data(0, vec!["x".to_string(), "y".to_string()]);

        assert_eq!(row(&table, 0), vec!["x", "y"]);
        assert_eq!(row(&table, 1), vec!["c", "d"]);
    }

    #[test]
    fn test_fill_row_with_data_wrong_size_ignored() {
        let mut table = make_table(vec![
            vec!["a", "b"],
        ]);

        table.fill_row_with_data(0, vec!["x".to_string()]); // Wrong size

        assert_eq!(row(&table, 0), vec!["a", "b"]); // Unchanged
    }

    #[test]
    fn test_fill_col_with_data() {
        let mut table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        table.fill_col_with_data(0, vec!["x".to_string(), "y".to_string()]);

        assert_eq!(row(&table, 0), vec!["x", "b"]);
        assert_eq!(row(&table, 1), vec!["y", "d"]);
    }

    #[test]
    fn test_fill_span_with_data() {
        let mut table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["g", "h", "i"],
        ]);

        table.fill_span_with_data(0, 0, vec![
            vec!["1".to_string(), "2".to_string()],
            vec!["3".to_string(), "4".to_string()],
        ]);

        assert_eq!(row(&table, 0), vec!["1", "2", "c"]);
        assert_eq!(row(&table, 1), vec!["3", "4", "f"]);
        assert_eq!(row(&table, 2), vec!["g", "h", "i"]);
    }

    // === TableView tests ===

    #[test]
    fn test_tableview_new() {
        let view = TableView::new();
        assert_eq!(view.cursor_row, 0);
        assert_eq!(view.cursor_col, 0);
        assert_eq!(view.viewport_row, 0);
        assert_eq!(view.viewport_col, 0);
    }

    #[test]
    fn test_tableview_navigation() {
        let mut view = TableView::new();
        let table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["g", "h", "i"],
        ]);

        view.move_right(&table);
        assert_eq!(view.cursor_col, 1);

        view.move_down(&table);
        assert_eq!(view.cursor_row, 1);

        view.move_left();
        assert_eq!(view.cursor_col, 0);

        view.move_up();
        assert_eq!(view.cursor_row, 0);
    }

    #[test]
    fn test_tableview_navigation_bounds() {
        let mut view = TableView::new();
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        // Can't go negative
        view.move_left();
        assert_eq!(view.cursor_col, 0);

        view.move_up();
        assert_eq!(view.cursor_row, 0);

        // Can't go past bounds
        view.cursor_col = 1;
        view.cursor_row = 1;

        view.move_right(&table);
        assert_eq!(view.cursor_col, 1);

        view.move_down(&table);
        assert_eq!(view.cursor_row, 1);
    }

    #[test]
    fn test_tableview_move_to_edges() {
        let mut view = TableView::new();
        let table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["g", "h", "i"],
        ]);

        view.cursor_col = 1;
        view.cursor_row = 1;

        view.move_to_first_col();
        assert_eq!(view.cursor_col, 0);

        view.move_to_last_col(&table);
        assert_eq!(view.cursor_col, 2);

        view.move_to_top();
        assert_eq!(view.cursor_row, 0);

        view.move_to_bottom(&table);
        assert_eq!(view.cursor_row, 2);
    }

    #[test]
    fn test_tableview_clamp_cursor() {
        let mut view = TableView::new();
        view.cursor_row = 100;
        view.cursor_col = 100;

        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
        ]);

        view.clamp_cursor(&table);

        assert_eq!(view.cursor_row, 1);
        assert_eq!(view.cursor_col, 1);
    }

    #[test]
    fn test_tableview_current_cell() {
        let view = TableView::new();
        let table = make_table(vec![
            vec!["hello", "world"],
        ]);

        assert_eq!(view.current_cell(&table), "hello");
    }

    #[test]
    fn test_tableview_update_col_widths() {
        let mut view = TableView::new();
        let table = make_table(vec![
            vec!["a", "longer", "x"],
            vec!["bb", "y", "shortest"],
        ]);

        view.update_col_widths(&table);

        assert_eq!(view.col_widths[0], 3); // min width is 3
        assert_eq!(view.col_widths[1], 6); // "longer"
        assert_eq!(view.col_widths[2], 8); // "shortest"
    }

    #[test]
    fn test_tableview_set_support() {
        let mut view = TableView::new();
        view.cursor_row = 5;
        view.cursor_col = 3;

        view.set_support();

        assert_eq!(view.support_row, 5);
        assert_eq!(view.support_col, 3);
    }

    #[test]
    fn test_tableview_yank_row() {
        let view = TableView::new();
        let table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
        ]);

        let row = view.yank_row(&table).unwrap();
        assert_eq!(row, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[test]
    fn test_tableview_yank_col() {
        let view = TableView::new();
        let table = make_table(vec![
            vec!["a", "b"],
            vec!["c", "d"],
            vec!["e", "f"],
        ]);

        let col = view.yank_col(&table).unwrap();
        assert_eq!(col, vec!["a".to_string(), "c".to_string(), "e".to_string()]);
    }

    #[test]
    fn test_tableview_yank_span() {
        let mut view = TableView::new();
        view.cursor_row = 0;
        view.cursor_col = 0;
        view.support_row = 1;
        view.support_col = 1;

        let table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["g", "h", "i"],
        ]);

        let span = view.yank_span(&table).unwrap();
        assert_eq!(span, vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["d".to_string(), "e".to_string()],
        ]);
    }

    #[test]
    fn test_tableview_is_selected_visual() {
        let mut view = TableView::new();
        view.cursor_row = 0;
        view.cursor_col = 0;
        view.support_row = 2;
        view.support_col = 2;

        assert!(view.is_selected(0, 0, Mode::Visual));
        assert!(view.is_selected(1, 1, Mode::Visual));
        assert!(view.is_selected(2, 2, Mode::Visual));
        assert!(!view.is_selected(3, 0, Mode::Visual));
        assert!(!view.is_selected(0, 3, Mode::Visual));
    }

    #[test]
    fn test_tableview_is_selected_visual_row() {
        let mut view = TableView::new();
        view.cursor_row = 1;
        view.cursor_col = 1;
        view.support_row = 2;
        view.support_col = 1;

        // In VisualRow, columns don't matter
        assert!(view.is_selected(1, 0, Mode::VisualRow));
        assert!(view.is_selected(1, 100, Mode::VisualRow));
        assert!(view.is_selected(2, 0, Mode::VisualRow));
        assert!(!view.is_selected(0, 0, Mode::VisualRow));
        assert!(!view.is_selected(3, 0, Mode::VisualRow));
    }

    #[test]
    fn test_tableview_is_selected_visual_col() {
        let mut view = TableView::new();
        view.cursor_row = 1;
        view.cursor_col = 1;
        view.support_row = 1;
        view.support_col = 2;

        // In VisualCol, rows don't matter
        assert!(view.is_selected(0, 1, Mode::VisualCol));
        assert!(view.is_selected(100, 1, Mode::VisualCol));
        assert!(view.is_selected(0, 2, Mode::VisualCol));
        assert!(!view.is_selected(0, 0, Mode::VisualCol));
        assert!(!view.is_selected(0, 3, Mode::VisualCol));
    }

    #[test]
    fn test_tableview_page_navigation() {
        let mut view = TableView::new();
        view.visible_rows = 10;

        let table = make_table(vec![vec!["x"]; 100]); // 100 rows

        view.page_down(&table);
        assert_eq!(view.cursor_row, 9);

        view.page_down(&table);
        assert_eq!(view.cursor_row, 18);

        view.page_up();
        assert_eq!(view.cursor_row, 9);

        view.half_page_down(&table);
        assert_eq!(view.cursor_row, 14);

        view.half_page_up();
        assert_eq!(view.cursor_row, 9);
    }

    #[test]
    fn test_tableview_expand_column() {
        let mut view = TableView::new();
        view.col_widths = vec![5, 5, 5];
        view.cursor_col = 1;

        view.expand_column(10);
        assert_eq!(view.col_widths[1], 10);

        // Shouldn't shrink
        view.expand_column(3);
        assert_eq!(view.col_widths[1], 10);
    }

    // === Sorting tests ===

    #[test]
    fn test_probe_column_type_numeric() {
        let table = make_table(vec![
            vec!["Name", "Score"],
            vec!["Alice", "95"],
            vec!["Bob", "87"],
            vec!["Carol", "92"],
        ]);

        // Column 0 is text (names)
        assert_eq!(table.probe_column_type(0, true), SortType::Text);
        // Column 1 is numeric (scores)
        assert_eq!(table.probe_column_type(1, true), SortType::Numeric);
    }

    #[test]
    fn test_probe_column_type_mixed() {
        let table = make_table(vec![
            vec!["ID", "Value"],
            vec!["1", "100"],
            vec!["2", "N/A"],
            vec!["3", "200"],
        ]);

        // Column 0 is numeric
        assert_eq!(table.probe_column_type(0, true), SortType::Numeric);
        // Column 1 is mixed but majority numeric
        assert_eq!(table.probe_column_type(1, true), SortType::Numeric);
    }

    #[test]
    fn test_probe_column_type_with_empty_cells() {
        let table = make_table(vec![
            vec!["Header"],
            vec!["10"],
            vec![""],
            vec!["20"],
            vec![""],
        ]);

        // Empty cells should be ignored; remaining are numeric
        assert_eq!(table.probe_column_type(0, true), SortType::Numeric);
    }

    #[test]
    fn test_probe_column_type_all_text() {
        let table = make_table(vec![
            vec!["Names"],
            vec!["Alice"],
            vec!["Bob"],
            vec!["Carol"],
        ]);

        assert_eq!(table.probe_column_type(0, true), SortType::Text);
    }

    #[test]
    fn test_get_sorted_row_indices_numeric_ascending() {
        let table = make_table(vec![
            vec!["Name", "Score"],
            vec!["Alice", "95"],
            vec!["Bob", "87"],
            vec!["Carol", "92"],
        ]);

        // Sort by score (column 1), ascending, skip header
        let indices = table.get_sorted_row_indices(1, SortDirection::Ascending, true);

        // Expected: header stays at 0, then Bob (87), Carol (92), Alice (95)
        assert_eq!(indices, vec![0, 2, 3, 1]);
    }

    #[test]
    fn test_get_sorted_row_indices_numeric_descending() {
        let table = make_table(vec![
            vec!["Name", "Score"],
            vec!["Alice", "95"],
            vec!["Bob", "87"],
            vec!["Carol", "92"],
        ]);

        // Sort by score (column 1), descending, skip header
        let indices = table.get_sorted_row_indices(1, SortDirection::Descending, true);

        // Expected: header stays at 0, then Alice (95), Carol (92), Bob (87)
        assert_eq!(indices, vec![0, 1, 3, 2]);
    }

    #[test]
    fn test_get_sorted_row_indices_text_ascending() {
        let table = make_table(vec![
            vec!["Name", "Score"],
            vec!["Carol", "92"],
            vec!["Alice", "95"],
            vec!["Bob", "87"],
        ]);

        // Sort by name (column 0), ascending, skip header
        let indices = table.get_sorted_row_indices(0, SortDirection::Ascending, true);

        // Expected: header stays at 0, then Alice, Bob, Carol
        assert_eq!(indices, vec![0, 2, 3, 1]);
    }

    #[test]
    fn test_get_sorted_row_indices_no_header() {
        let table = make_table(vec![
            vec!["Carol", "92"],
            vec!["Alice", "95"],
            vec!["Bob", "87"],
        ]);

        // Sort by name (column 0), ascending, NO header skip
        let indices = table.get_sorted_row_indices(0, SortDirection::Ascending, false);

        // Expected: Alice, Bob, Carol
        assert_eq!(indices, vec![1, 2, 0]);
    }

    #[test]
    fn test_get_sorted_row_indices_with_non_numeric() {
        let table = make_table(vec![
            vec!["ID", "Value"],
            vec!["1", "100"],
            vec!["2", "N/A"],
            vec!["3", "50"],
        ]);

        // Sort by value (column 1), ascending
        // N/A should go to the end
        let indices = table.get_sorted_row_indices(1, SortDirection::Ascending, true);

        // 50, 100, N/A
        assert_eq!(indices, vec![0, 3, 1, 2]);
    }

    #[test]
    fn test_get_sorted_col_indices() {
        let table = make_table(vec![
            vec!["C", "A", "B"],
            vec!["3", "1", "2"],
        ]);

        // Sort columns by row 0 (text), ascending
        let indices = table.get_sorted_col_indices(0, SortDirection::Ascending, false);

        // A, B, C
        assert_eq!(indices, vec![1, 2, 0]);
    }

    #[test]
    fn test_get_sorted_col_indices_numeric() {
        let table = make_table(vec![
            vec!["30", "10", "20"],
            vec!["C", "A", "B"],
        ]);

        // Sort columns by row 0 (numeric), ascending
        let indices = table.get_sorted_col_indices(0, SortDirection::Ascending, false);

        // 10, 20, 30
        assert_eq!(indices, vec![1, 2, 0]);
    }

    #[test]
    fn test_reorder_rows() {
        let mut table = make_table(vec![
            vec!["a", "1"],
            vec!["b", "2"],
            vec!["c", "3"],
        ]);

        let old = table.reorder_rows(&[2, 0, 1]);

        assert_eq!(row(&table, 0), vec!["c", "3"]);
        assert_eq!(row(&table, 1), vec!["a", "1"]);
        assert_eq!(row(&table, 2), vec!["b", "2"]);

        // Old data should be preserved for undo
        assert_eq!(old[0], vec!["a", "1"]);
        assert_eq!(old[1], vec!["b", "2"]);
        assert_eq!(old[2], vec!["c", "3"]);
    }

    #[test]
    fn test_reorder_cols() {
        let mut table = make_table(vec![
            vec!["a", "b", "c"],
            vec!["1", "2", "3"],
        ]);

        let old = table.reorder_cols(&[2, 0, 1]);

        assert_eq!(row(&table, 0), vec!["c", "a", "b"]);
        assert_eq!(row(&table, 1), vec!["3", "1", "2"]);

        // Old data should be preserved
        assert_eq!(old[0], vec!["a", "b", "c"]);
    }

    #[test]
    fn test_sort_case_insensitive() {
        let table = make_table(vec![
            vec!["name"],
            vec!["Banana"],
            vec!["apple"],
            vec!["Cherry"],
        ]);

        let indices = table.get_sorted_row_indices(0, SortDirection::Ascending, true);

        // apple, Banana, Cherry (case-insensitive)
        assert_eq!(indices, vec![0, 2, 1, 3]);
    }

    #[test]
    fn test_sort_negative_numbers() {
        let table = make_table(vec![
            vec!["value"],
            vec!["-10"],
            vec!["5"],
            vec!["-3"],
            vec!["0"],
        ]);

        let indices = table.get_sorted_row_indices(0, SortDirection::Ascending, true);

        // -10, -3, 0, 5
        assert_eq!(indices, vec![0, 1, 3, 4, 2]);
    }

    #[test]
    fn test_sort_float_numbers() {
        let table = make_table(vec![
            vec!["value"],
            vec!["1.5"],
            vec!["1.05"],
            vec!["1.25"],
        ]);

        let indices = table.get_sorted_row_indices(0, SortDirection::Ascending, true);

        // 1.05, 1.25, 1.5
        assert_eq!(indices, vec![0, 2, 3, 1]);
    }

    // === Large-scale sorting tests ===

    /// Helper to create a large table with random-ish numeric values for sorting tests
    fn make_large_sortable_table(num_rows: usize) -> Table {
        // Create rows with values that will sort in a known order
        // Use a simple formula: value = (num_rows - i) to create reverse order
        let rows: Vec<Vec<String>> = (0..num_rows)
            .map(|i| vec![
                format!("{}", num_rows - i),  // Numeric col (reverse order)
                format!("name{:05}", i),      // Text col (forward order)
            ])
            .collect();
        Table::new(rows)
    }

    #[test]
    fn test_sort_rows_bulk_large_ascending() {
        // 3000 rows spanning 3 chunks
        let mut table = make_large_sortable_table(3000);
        assert_eq!(table.row_count(), 3000);

        // First column has values 3000, 2999, 2998, ..., 1
        // After ascending sort, should be 1, 2, 3, ..., 3000
        assert_eq!(table.get_cell(0, 0), Some(&"3000".to_string()));
        assert_eq!(table.get_cell(2999, 0), Some(&"1".to_string()));

        let old_data = table.sort_rows_by_column(0, SortDirection::Ascending, false);
        assert!(old_data.is_some(), "Sort should have changed the order");

        // Verify sorted order
        for i in 0..3000 {
            let expected = format!("{}", i + 1);
            assert_eq!(table.get_cell(i, 0), Some(&expected),
                "Row {} should have value {}", i, expected);
        }

        // Verify row count unchanged
        assert_eq!(table.row_count(), 3000);
    }

    #[test]
    fn test_sort_rows_bulk_large_descending() {
        // Create table with ascending values
        let rows: Vec<Vec<String>> = (0..3000)
            .map(|i| vec![format!("{}", i + 1)])
            .collect();
        let mut table = Table::new(rows);

        // First column has values 1, 2, 3, ..., 3000
        assert_eq!(table.get_cell(0, 0), Some(&"1".to_string()));
        assert_eq!(table.get_cell(2999, 0), Some(&"3000".to_string()));

        let old_data = table.sort_rows_by_column(0, SortDirection::Descending, false);
        assert!(old_data.is_some());

        // After descending sort: 3000, 2999, ..., 1
        for i in 0..3000 {
            let expected = format!("{}", 3000 - i);
            assert_eq!(table.get_cell(i, 0), Some(&expected),
                "Row {} should have value {}", i, expected);
        }
    }

    #[test]
    fn test_sort_rows_bulk_large_with_header() {
        // Create table with header row
        let mut rows: Vec<Vec<String>> = vec![vec!["Value".to_string()]];
        rows.extend((0..2999).map(|i| vec![format!("{}", 2999 - i)]));
        let mut table = Table::new(rows);

        assert_eq!(table.row_count(), 3000);
        assert_eq!(table.get_cell(0, 0), Some(&"Value".to_string()));
        assert_eq!(table.get_cell(1, 0), Some(&"2999".to_string()));

        let old_data = table.sort_rows_by_column(0, SortDirection::Ascending, true);
        assert!(old_data.is_some());

        // Header should remain at row 0
        assert_eq!(table.get_cell(0, 0), Some(&"Value".to_string()));

        // Data rows should be sorted: 1, 2, 3, ..., 2999
        for i in 1..3000 {
            let expected = format!("{}", i);
            assert_eq!(table.get_cell(i, 0), Some(&expected),
                "Row {} should have value {}", i, expected);
        }
    }

    #[test]
    fn test_sort_rows_bulk_large_text() {
        // Create table with text values that span chunks
        let rows: Vec<Vec<String>> = (0..3000)
            .map(|i| vec![format!("item{:05}", 2999 - i)])
            .collect();
        let mut table = Table::new(rows);

        // Should be: item02999, item02998, ..., item00000
        assert_eq!(table.get_cell(0, 0), Some(&"item02999".to_string()));
        assert_eq!(table.get_cell(2999, 0), Some(&"item00000".to_string()));

        let old_data = table.sort_rows_by_column(0, SortDirection::Ascending, false);
        assert!(old_data.is_some());

        // After sort: item00000, item00001, ..., item02999
        for i in 0..3000 {
            let expected = format!("item{:05}", i);
            assert_eq!(table.get_cell(i, 0), Some(&expected),
                "Row {} should have value {}", i, expected);
        }
    }

    #[test]
    fn test_sort_rows_bulk_already_sorted() {
        // Create already sorted table
        let rows: Vec<Vec<String>> = (0..3000)
            .map(|i| vec![format!("{}", i + 1)])
            .collect();
        let mut table = Table::new(rows);

        // Should return None since already sorted
        let old_data = table.sort_rows_by_column(0, SortDirection::Ascending, false);
        assert!(old_data.is_none(), "Already sorted table should return None");
    }

    #[test]
    fn test_sort_preserves_row_integrity() {
        // Create table with multiple columns to verify entire rows move together
        let rows: Vec<Vec<String>> = (0..3000)
            .map(|i| vec![
                format!("{}", 3000 - i),           // Sort key (reverse)
                format!("row{}", i),               // Row identifier
                format!("data{}", i * 2),          // Associated data
            ])
            .collect();
        let mut table = Table::new(rows);

        table.sort_rows_by_column(0, SortDirection::Ascending, false);

        // Verify row integrity - each row's columns should still correspond
        for i in 0..3000 {
            let sort_key = table.get_cell(i, 0).unwrap();
            let row_id = table.get_cell(i, 1).unwrap();
            let data = table.get_cell(i, 2).unwrap();

            // sort_key should be i+1 (after ascending sort)
            assert_eq!(sort_key, &format!("{}", i + 1));

            // Original row index was (3000 - (i+1)) = 2999 - i
            let orig_idx = 2999 - i;
            assert_eq!(row_id, &format!("row{}", orig_idx),
                "Row {} should have id row{}", i, orig_idx);
            assert_eq!(data, &format!("data{}", orig_idx * 2),
                "Row {} should have data{}", i, orig_idx * 2);
        }
    }

    #[test]
    fn test_reorder_rows_bulk_returns_old_data() {
        let mut table = make_large_sortable_table(3000);

        // Get original first and last values
        let orig_first = table.get_cell(0, 0).unwrap().clone();
        let orig_last = table.get_cell(2999, 0).unwrap().clone();

        // Create reverse order
        let new_order: Vec<usize> = (0..3000).rev().collect();
        let old_data = table.reorder_rows_bulk(&new_order);

        // Verify old_data contains original values
        assert_eq!(old_data.len(), 3000);
        assert_eq!(old_data[0][0], orig_first);
        assert_eq!(old_data[2999][0], orig_last);

        // Verify table is now reversed
        assert_eq!(table.get_cell(0, 0), Some(&orig_last));
        assert_eq!(table.get_cell(2999, 0), Some(&orig_first));
    }

    // === Bulk row operations ===

    #[test]
    fn test_delete_rows_bulk_single_chunk() {
        let mut table = make_table(vec![
            vec!["a", "1"],
            vec!["b", "2"],
            vec!["c", "3"],
            vec!["d", "4"],
            vec!["e", "5"],
        ]);

        let deleted = table.delete_rows_bulk(1, 2);

        assert_eq!(deleted.len(), 2);
        assert_eq!(deleted[0], vec!["b", "2"]);
        assert_eq!(deleted[1], vec!["c", "3"]);
        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 0), vec!["a", "1"]);
        assert_eq!(row(&table, 1), vec!["d", "4"]);
        assert_eq!(row(&table, 2), vec!["e", "5"]);
    }

    #[test]
    fn test_delete_rows_bulk_at_start() {
        let mut table = make_table(vec![
            vec!["a"],
            vec!["b"],
            vec!["c"],
            vec!["d"],
        ]);

        let deleted = table.delete_rows_bulk(0, 2);

        assert_eq!(deleted.len(), 2);
        // Verify deleted rows are in correct order
        assert_eq!(deleted[0], vec!["a"]);
        assert_eq!(deleted[1], vec!["b"]);
        assert_eq!(table.row_count(), 2);
        assert_eq!(row(&table, 0), vec!["c"]);
        assert_eq!(row(&table, 1), vec!["d"]);
    }

    #[test]
    fn test_delete_rows_bulk_order_preserved() {
        // Create a table with numbered rows to verify order
        let mut table = make_table(vec![
            vec!["row0"],
            vec!["row1"],
            vec!["row2"],
            vec!["row3"],
            vec!["row4"],
            vec!["row5"],
            vec!["row6"],
            vec!["row7"],
        ]);

        // Delete middle rows
        let deleted = table.delete_rows_bulk(2, 4);

        assert_eq!(deleted.len(), 4);
        // Verify deleted rows are returned in correct order
        assert_eq!(deleted[0], vec!["row2"]);
        assert_eq!(deleted[1], vec!["row3"]);
        assert_eq!(deleted[2], vec!["row4"]);
        assert_eq!(deleted[3], vec!["row5"]);

        // Verify remaining rows
        assert_eq!(table.row_count(), 4);
        assert_eq!(row(&table, 0), vec!["row0"]);
        assert_eq!(row(&table, 1), vec!["row1"]);
        assert_eq!(row(&table, 2), vec!["row6"]);
        assert_eq!(row(&table, 3), vec!["row7"]);
    }

    /// Helper to create a large table with numbered rows for cross-chunk testing
    fn make_large_table(num_rows: usize) -> Table {
        let rows: Vec<Vec<String>> = (0..num_rows)
            .map(|i| vec![format!("row{}", i), format!("val{}", i)])
            .collect();
        Table::new(rows)
    }

    #[test]
    fn test_delete_rows_bulk_cross_chunk_middle() {
        // 3000 rows = 3 chunks (0-1023, 1024-2047, 2048-2999)
        let mut table = make_large_table(3000);
        assert_eq!(table.row_count(), 3000);

        // Delete 2000 rows from the middle (rows 500-2499)
        // This spans all 3 chunks
        let deleted = table.delete_rows_bulk(500, 2000);

        assert_eq!(deleted.len(), 2000);
        assert_eq!(table.row_count(), 1000);

        // Verify deleted rows are in correct order
        for i in 0..2000 {
            assert_eq!(deleted[i][0], format!("row{}", 500 + i),
                "deleted row {} should be row{}", i, 500 + i);
        }

        // Verify remaining rows
        for i in 0..500 {
            assert_eq!(row(&table, i)[0], format!("row{}", i),
                "remaining row {} should be row{}", i, i);
        }
        for i in 0..500 {
            assert_eq!(row(&table, 500 + i)[0], format!("row{}", 2500 + i),
                "remaining row {} should be row{}", 500 + i, 2500 + i);
        }
    }

    #[test]
    fn test_delete_rows_bulk_cross_chunk_top() {
        // 3000 rows = 3 chunks
        let mut table = make_large_table(3000);

        // Delete 2000 rows from the top (rows 0-1999)
        // This spans chunks 0 and 1 completely, plus part of chunk 2
        let deleted = table.delete_rows_bulk(0, 2000);

        assert_eq!(deleted.len(), 2000);
        assert_eq!(table.row_count(), 1000);

        // Verify deleted rows are in correct order
        for i in 0..2000 {
            assert_eq!(deleted[i][0], format!("row{}", i),
                "deleted row {} should be row{}", i, i);
        }

        // Verify remaining rows (rows 2000-2999 should now be at 0-999)
        for i in 0..1000 {
            assert_eq!(row(&table, i)[0], format!("row{}", 2000 + i),
                "remaining row {} should be row{}", i, 2000 + i);
        }
    }

    #[test]
    fn test_delete_rows_bulk_cross_chunk_end() {
        // 3000 rows = 3 chunks
        let mut table = make_large_table(3000);

        // Delete 2000 rows from the end (rows 1000-2999)
        // This spans parts of chunk 0, all of chunk 1, and all of chunk 2
        let deleted = table.delete_rows_bulk(1000, 2000);

        assert_eq!(deleted.len(), 2000);
        assert_eq!(table.row_count(), 1000);

        // Verify deleted rows are in correct order
        for i in 0..2000 {
            assert_eq!(deleted[i][0], format!("row{}", 1000 + i),
                "deleted row {} should be row{}", i, 1000 + i);
        }

        // Verify remaining rows (rows 0-999 should still be there)
        for i in 0..1000 {
            assert_eq!(row(&table, i)[0], format!("row{}", i),
                "remaining row {} should be row{}", i, i);
        }
    }

    #[test]
    fn test_delete_rows_bulk_exactly_two_chunks() {
        // Test deletion that spans exactly 2 chunks with no middle chunks
        let mut table = make_large_table(3000);

        // Delete rows 1000-1100 (spans chunk 0 end and chunk 1 start)
        let deleted = table.delete_rows_bulk(1000, 101);

        assert_eq!(deleted.len(), 101);
        assert_eq!(table.row_count(), 2899);

        // Verify deleted rows are in correct order
        for i in 0..101 {
            assert_eq!(deleted[i][0], format!("row{}", 1000 + i),
                "deleted row {} should be row{}", i, 1000 + i);
        }
    }

    #[test]
    fn test_delete_rows_bulk_at_end() {
        let mut table = make_table(vec![
            vec!["a"],
            vec!["b"],
            vec!["c"],
            vec!["d"],
        ]);

        let deleted = table.delete_rows_bulk(2, 2);

        assert_eq!(deleted.len(), 2);
        assert_eq!(table.row_count(), 2);
        assert_eq!(row(&table, 0), vec!["a"]);
        assert_eq!(row(&table, 1), vec!["b"]);
    }

    #[test]
    fn test_delete_rows_bulk_all_rows_clears() {
        let mut table = make_table(vec![
            vec!["a", "1"],
            vec!["b", "2"],
            vec!["c", "3"],
        ]);

        let deleted = table.delete_rows_bulk(0, 3);

        assert_eq!(deleted.len(), 3);
        assert_eq!(table.row_count(), 1);
        assert_eq!(row(&table, 0), vec!["", ""]);
    }

    #[test]
    fn test_delete_rows_bulk_exceeds_count() {
        let mut table = make_table(vec![
            vec!["a"],
            vec!["b"],
            vec!["c"],
        ]);

        let deleted = table.delete_rows_bulk(1, 100);

        assert_eq!(deleted.len(), 2);
        assert_eq!(table.row_count(), 1);
        assert_eq!(row(&table, 0), vec!["a"]);
    }

    #[test]
    fn test_insert_rows_bulk_empty() {
        let mut table = make_table(vec![
            vec!["a", "1"],
            vec!["b", "2"],
        ]);

        table.insert_rows_bulk(1, 2);

        assert_eq!(table.row_count(), 4);
        assert_eq!(row(&table, 0), vec!["a", "1"]);
        assert_eq!(row(&table, 1), vec!["", ""]);
        assert_eq!(row(&table, 2), vec!["", ""]);
        assert_eq!(row(&table, 3), vec!["b", "2"]);
    }

    #[test]
    fn test_insert_rows_with_data_bulk() {
        let mut table = make_table(vec![
            vec!["a", "1"],
            vec!["d", "4"],
        ]);

        table.insert_rows_with_data_bulk(1, vec![
            vec!["b".to_string(), "2".to_string()],
            vec!["c".to_string(), "3".to_string()],
        ]);

        assert_eq!(table.row_count(), 4);
        assert_eq!(row(&table, 0), vec!["a", "1"]);
        assert_eq!(row(&table, 1), vec!["b", "2"]);
        assert_eq!(row(&table, 2), vec!["c", "3"]);
        assert_eq!(row(&table, 3), vec!["d", "4"]);
    }

    #[test]
    fn test_insert_rows_bulk_at_end() {
        let mut table = make_table(vec![
            vec!["a"],
            vec!["b"],
        ]);

        table.insert_rows_with_data_bulk(2, vec![
            vec!["c".to_string()],
            vec!["d".to_string()],
        ]);

        assert_eq!(table.row_count(), 4);
        assert_eq!(row(&table, 2), vec!["c"]);
        assert_eq!(row(&table, 3), vec!["d"]);
    }

    #[test]
    fn test_insert_rows_bulk_pads_short_rows() {
        let mut table = make_table(vec![
            vec!["a", "b", "c"],
        ]);

        table.insert_rows_with_data_bulk(1, vec![
            vec!["x".to_string()], // Short row
        ]);

        assert_eq!(row(&table, 1), vec!["x", "", ""]);
    }

    #[test]
    fn test_fill_rows_bulk() {
        let mut table = make_table(vec![
            vec!["a", "1"],
            vec!["b", "2"],
            vec!["c", "3"],
        ]);

        table.fill_rows_with_data_bulk(0, vec![
            vec!["x".to_string(), "7".to_string()],
            vec!["y".to_string(), "8".to_string()],
        ]);

        assert_eq!(row(&table, 0), vec!["x", "7"]);
        assert_eq!(row(&table, 1), vec!["y", "8"]);
        assert_eq!(row(&table, 2), vec!["c", "3"]);
    }

    #[test]
    fn test_fill_rows_bulk_expands_table() {
        let mut table = make_table(vec![
            vec!["a", "1"],
        ]);

        table.fill_rows_with_data_bulk(0, vec![
            vec!["x".to_string(), "7".to_string()],
            vec!["y".to_string(), "8".to_string()],
            vec!["z".to_string(), "9".to_string()],
        ]);

        assert_eq!(table.row_count(), 3);
        assert_eq!(row(&table, 0), vec!["x", "7"]);
        assert_eq!(row(&table, 1), vec!["y", "8"]);
        assert_eq!(row(&table, 2), vec!["z", "9"]);
    }

    #[test]
    fn test_get_rows_cloned() {
        let table = make_table(vec![
            vec!["a", "1"],
            vec!["b", "2"],
            vec!["c", "3"],
            vec!["d", "4"],
        ]);

        let rows = table.get_rows_cloned(1, 2);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["b", "2"]);
        assert_eq!(rows[1], vec!["c", "3"]);
    }

    #[test]
    fn test_get_rows_cloned_clamps_count() {
        let table = make_table(vec![
            vec!["a"],
            vec!["b"],
            vec!["c"],
        ]);

        let rows = table.get_rows_cloned(1, 100);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["b"]);
        assert_eq!(rows[1], vec!["c"]);
    }

    #[test]
    fn test_probe_row_type() {
        let table = make_table(vec![
            vec!["Name", "Alice", "Bob", "Carol"],
            vec!["Score", "95", "87", "92"],
        ]);

        // Row 0 is text (names)
        assert_eq!(table.probe_row_type(0, true), SortType::Text);
        // Row 1 is numeric (scores)
        assert_eq!(table.probe_row_type(1, true), SortType::Numeric);
    }
}
