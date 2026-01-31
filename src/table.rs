use std::cmp;

use crate::mode::Mode;

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

        let mut indices: Vec<usize> = if skip_header {
            vec![0]
        } else {
            Vec::new()
        };

        match sort_type {
            SortType::Numeric => {
                let mut keyed: Vec<(usize, f64)> = (start_row..self.row_count())
                    .map(|row| {
                        let s = crate::format::parse_numeric(self.get_cell(row, sort_col).map(|x| x.as_str().trim()).unwrap_or("")).unwrap_or(f64::NAN);
                        (row, s)
                        }).collect();

                match direction {
                    SortDirection::Ascending => keyed.sort_unstable_by(|(idx_a,num_a), (idx_b,num_b)| -> std::cmp::Ordering { 
                            match (num_a.is_nan(), num_b.is_nan()) {
                                (true, true) => std::cmp::Ordering::Equal, // Both NaN
                                (true, false) => std::cmp::Ordering::Greater, // NaN goes last
                                (false, true) => std::cmp::Ordering::Less,
                                (false, false) => num_a.partial_cmp(&num_b).unwrap_or(std::cmp::Ordering::Equal),
                            }.then(idx_a.cmp(&idx_b))
                        }),
                    SortDirection::Descending => keyed.sort_unstable_by(|(idx_a,num_a), (idx_b,num_b)| -> std::cmp::Ordering { 
                            match (num_a.is_nan(), num_b.is_nan()) {
                                (true, true) => std::cmp::Ordering::Equal, // Both NaN
                                (true, false) => std::cmp::Ordering::Greater, // NaN goes last
                                (false, true) => std::cmp::Ordering::Less,
                                (false, false) => num_a.partial_cmp(&num_b).unwrap_or(std::cmp::Ordering::Equal).reverse(),
                            }.then(idx_a.cmp(&idx_b))
                        }),

                }

                indices.extend(keyed.into_iter().map(|(row,_)| row));
            },
            SortType::Text => {
                let mut keyed: Vec<(usize, String)> = (start_row..self.row_count())
                    .map(|row| {
                        let s = self.get_cell(row, sort_col).unwrap_or(&String::new()).to_lowercase().trim().to_owned();
                        (row, s)
                        }).collect();

                match direction {
                    SortDirection::Ascending => keyed.sort_unstable_by(|(i,a), (j,b)|  a.cmp(&b).then(i.cmp(&j))),
                    SortDirection::Descending => keyed.sort_unstable_by(|(i,a), (j,b)|  a.cmp(&b).reverse().then(i.cmp(&j)))
                }

                indices.extend(keyed.into_iter().map(|(row,_)| row));
            }
        }

        indices
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

        let mut indices: Vec<usize> = if skip_first_col {
            vec![0]
        } else {
            Vec::new()
        };
        
        match sort_type {
            SortType::Numeric => {
                let mut keyed: Vec<(usize, f64)> = (start_col..self.col_count())
                    .map(|col| {
                        let s = crate::format::parse_numeric(self.get_cell(sort_row, col).map(|x| x.as_str().trim()).unwrap_or("")).unwrap_or(f64::NAN);
                        (col, s)
                        }).collect();

                match direction {
                    SortDirection::Ascending => keyed.sort_unstable_by(|(idx_a,num_a), (idx_b,num_b)| -> std::cmp::Ordering { 
                            match (num_a.is_nan(), num_b.is_nan()) {
                                (true, true) => std::cmp::Ordering::Equal, // Both NaN
                                (true, false) => std::cmp::Ordering::Greater, // NaN goes last
                                (false, true) => std::cmp::Ordering::Less,
                                (false, false) => num_a.partial_cmp(&num_b).unwrap_or(std::cmp::Ordering::Equal),
                            }.then(idx_a.cmp(&idx_b))
                        }),
                    SortDirection::Descending => keyed.sort_unstable_by(|(idx_a,num_a), (idx_b,num_b)| -> std::cmp::Ordering { 
                            match (num_a.is_nan(), num_b.is_nan()) {
                                (true, true) => std::cmp::Ordering::Equal, // Both NaN
                                (true, false) => std::cmp::Ordering::Greater, // NaN goes last
                                (false, true) => std::cmp::Ordering::Less,
                                (false, false) => num_a.partial_cmp(&num_b).unwrap_or(std::cmp::Ordering::Equal).reverse(),
                            }.then(idx_a.cmp(&idx_b))
                        }),

                }

                indices.extend(keyed.into_iter().map(|(col,_)| col));
            },
            SortType::Text => {
                let mut keyed: Vec<(usize, String)> = (start_col..self.col_count())
                    .map(|col| {
                        let s = self.get_cell(sort_row, col).unwrap_or(&String::new()).to_lowercase().trim().to_owned();
                        (col, s)
                        }).collect();

                match direction {
                    SortDirection::Ascending => keyed.sort_unstable_by(|(i,a), (j,b)|  a.cmp(&b).then(i.cmp(&j))),
                    SortDirection::Descending => keyed.sort_unstable_by(|(i,a), (j,b)|  a.cmp(&b).reverse().then(i.cmp(&j))),
                }

                indices.extend(keyed.into_iter().map(|(col,_)| col));
            }
        }
    

        indices
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
}
