use rayon::prelude::*;

use std::cmp;

/// Number of rows per chunk for memory-efficient storage
pub const CHUNK_SIZE: usize = 1024;

/// Threshold for using parallel processing (rows * cols)
const PARALLEL_THRESHOLD: usize = 10_000;

/// Pure data structure for the table with chunked row storage
#[derive(Debug, Clone)]
pub struct Table {
    /// Rows stored in fixed-size chunks for memory efficiency
    pub(crate) chunks: Vec<Vec<Vec<String>>>,
    /// Total number of rows
    pub(crate) total_rows: usize,
    /// Number of columns
    col_count: usize,
    /// Cached column widths (max length of any cell in each column)
    pub(crate) col_widths: Vec<usize>,
    /// Whether col_widths needs full recompute
    col_widths_dirty: bool,
    pub max_col_width: usize
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

    /// Clone all rows into a flat Vec (for undo/redo operations)
    pub fn clone_all_rows(&self) -> Vec<Vec<String>> {
        self.chunks.iter().flat_map(|chunk| chunk.iter().cloned()).collect()
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
            max_col_width: 30
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
            max_col_width: 30,
        };
        table.recompute_col_widths();
        table
    }

    /// Create a table from pre-chunked data, deferring column width computation
    /// until first render. Use this for faster file loading.
    #[allow(dead_code)]
    pub fn from_chunks_lazy(chunks: Vec<Vec<Vec<String>>>, col_count: usize) -> Self {
        let total_rows: usize = chunks.iter().map(|c| c.len()).sum();
        Self {
            chunks,
            total_rows,
            col_count,
            col_widths: Vec::new(),
            col_widths_dirty: true,
            max_col_width: 30,
        }
    }

    /// Get cached column widths, recomputing if dirty
    pub fn col_widths(&mut self) -> &[usize] {
        if self.col_widths_dirty {
            self.recompute_col_widths();
        }
        &self.col_widths
    }

    /// Force recompute of column widths
    /// Uses parallel processing for large tables
    pub fn recompute_col_widths(&mut self) {
        let size = self.total_rows * self.col_count;

        if size >= PARALLEL_THRESHOLD && self.col_count > 1 {
            // Parallel: compute each column's max width in parallel
            // Collect all cells first to enable parallel iteration
            let all_rows: Vec<&Vec<String>> = self.rows_iter().collect();

            self.col_widths = (0..self.col_count)
                .into_par_iter()
                .map(|col| {
                    all_rows
                        .iter()
                        .filter_map(|row| row.get(col))
                        .map(|s| crate::util::display_width(s))
                        .max()
                        .unwrap_or(3)
                        .max(3)
                        .min(self.max_col_width)
                })
                .collect();
        } else {
            // Sequential for small tables
            self.col_widths = (0..self.col_count)
                .map(|col| {
                    self.rows_iter()
                        .filter_map(|row| row.get(col))
                        .map(|s| crate::util::display_width(s))
                        .max()
                        .unwrap_or(3)
                        .max(3)
                        .min(self.max_col_width)
                })
                .collect();
        }
        self.col_widths_dirty = false;
    }

    /// Mark column widths as needing recompute
    #[inline]
    pub(crate) fn mark_widths_dirty(&mut self) {
        self.col_widths_dirty = true;
    }

    /// Update width for a single column (when cell changes)
    #[inline]
    fn update_col_width(&mut self, col: usize, new_len: usize) {
        if col < self.col_widths.len() {
            self.col_widths[col] = cmp::min(self.col_widths[col].max(new_len).max(3), self.max_col_width)
        }
    }

    /// Expand a column width if the new length is larger (public API for insert mode)
    #[inline]
    pub fn expand_col_width(&mut self, col: usize, new_len: usize) {
        self.update_col_width(col, new_len);
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
                    let new_width = crate::util::display_width(&value);
                    *cell = value;

                    // Update column width incrementally (only grows, never shrinks)
                    self.update_col_width(col, new_width);
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
                self.update_col_width(col, crate::util::display_width(val));
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

    /// Get multiple contiguous rows as cloned data (for clipboard/undo)
    pub fn get_rows_cloned(&self, start_idx: usize, count: usize) -> Vec<Vec<String>> {
        if start_idx + count > self.total_rows || count == 0 {
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

    /// Get a cloned copy of a column (for transactions/clipboard)
    pub fn get_cols_cloned(&self, start_idx: usize, count: usize) -> Vec<Vec<String>> {
        if start_idx + count > self.col_count || count == 0 {
            return Vec::new();
        }

        let end_idx = cmp::min(start_idx+count, self.col_count).saturating_sub(1);

        (0..self.total_rows)
            .map(|r| {
                (start_idx..=end_idx)
                    .map(|c| self.get_cell(r,c).cloned().unwrap_or_default())
                    .collect()
            }).collect()
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
            self.update_col_width(col, crate::util::display_width(val));
        }
        self.insert_row_internal(idx, row);
    }

    pub fn insert_col_with_data(&mut self, idx: usize, col: Vec<String>) {
        let max_width = col.iter().map(|s| crate::util::display_width(s)).max().unwrap_or(0).max(3);
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
}

impl Default for Table {
    fn default() -> Self {
        Table {
            chunks: vec![vec![vec![String::new()]]],
            total_rows: 1,
            col_count: 1,
            col_widths: vec![3],
            col_widths_dirty: false,
            max_col_width: 30,
        }
    }
}

