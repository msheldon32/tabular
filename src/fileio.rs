use std::path::PathBuf;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};

use crate::table::{Table, CHUNK_SIZE};

/// Detected file format
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileFormat {
    Csv,
    Tsv,
}

impl FileFormat {
    /// Detect format from file extension
    fn from_extension(path: &PathBuf) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "csv" => Some(FileFormat::Csv),
            "tsv" => Some(FileFormat::Tsv),
            _ => None
        }
    }

    /// Get the delimiter for CSV-like formats
    fn delimiter(&self) -> Option<u8> {
        match self {
            FileFormat::Csv => Some(b','),
            FileFormat::Tsv => Some(b'\t'),
            _ => None,
        }
    }
}

/// Result of loading a file, including any warnings
pub struct LoadResult {
    pub table: Table,
    pub warnings: Vec<String>,
}

pub struct FileIO {
    pub file_path: Option<PathBuf>,
    format: Option<FileFormat>,
    max_dim: (usize, usize)
}

impl FileIO {
    pub fn new(file_path: Option<PathBuf>) -> io::Result<Self> {
        let format = file_path.as_ref().and_then(FileFormat::from_extension);

        let max_dim = (10000000,10000000);
        Ok(Self { file_path, format, max_dim })
    }

    pub fn file_name(&self) -> String {
        self.file_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    }

    pub fn format(&self) -> Option<FileFormat> {
        self.format
    }

    /// Load table from file, returning warnings about any modifications
    pub fn load_table(&self) -> io::Result<LoadResult> {
        if self.file_path.is_none() {
            return Ok(LoadResult {
                table: Table::new(vec![vec![String::new()]]),
                warnings: Vec::new(),
            });
        }

        match self.format {
            Some(FileFormat::Csv) | Some(FileFormat::Tsv) => self.read_csv(),
            None => {
                // Default to CSV for unknown extensions
                self.read_csv()
            }
        }
    }

    /// Write table to file
    pub fn write(&self, table: &Table) -> io::Result<()> {
        if self.file_path.is_none() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file path specified"));
        }

        match self.format {
            Some(FileFormat::Csv) | Some(FileFormat::Tsv) => self.write_csv(table),
            None => self.write_csv(table),
        }
    }

    // === CSV/TSV ===

    fn read_csv(&self) -> io::Result<LoadResult> {
        let path = self.file_path.as_ref().ok_or(io::ErrorKind::NotFound)?;
        let delim = self.format.and_then(|f| f.delimiter()).unwrap_or(b',');

        // Check if file exists - if not, create empty table
        if !path.exists() {
            return Ok(LoadResult {
                table: Table::new(vec![vec![String::new(); 5]; 10]),
                warnings: vec![format!("New file: {}", path.display())],
            });
        }

        let file = File::open(path)?;
        let reader = BufReader::with_capacity(1 << 20, file); // 1 MB

        let mut csv_reader = csv::ReaderBuilder::new()
            .delimiter(delim)
            .has_headers(false)
            .flexible(true)
            .trim(csv::Trim::Fields)
            .from_reader(reader);

        // Stream directly into chunks to avoid intermediate Vec allocation
        let mut chunks: Vec<Vec<Vec<String>>> = Vec::new();
        let mut current_chunk: Vec<Vec<String>> = Vec::with_capacity(CHUNK_SIZE);
        let mut max_cols: usize = 0;
        let mut needs_padding = false;
        let mut row_no: usize = 0;

        for result in csv_reader.records() {
            let record = result.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();

            if row_no > self.max_dim.0 || row.len() > self.max_dim.1 {
                return Err(io::Error::from(io::ErrorKind::FileTooLarge));
            }

            // Track if padding will be needed (O(1) instead of O(n²))
            if row.len() > max_cols {
                if max_cols > 0 {
                    needs_padding = true;
                }
                max_cols = row.len();
            } else if row.len() < max_cols {
                needs_padding = true;
            }

            current_chunk.push(row);
            row_no += 1;

            // Flush full chunk
            if current_chunk.len() == CHUNK_SIZE {
                chunks.push(std::mem::take(&mut current_chunk));
                current_chunk = Vec::with_capacity(CHUNK_SIZE);
            }
        }

        // Push remaining rows
        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        // Handle empty file
        if chunks.is_empty() {
            chunks.push(vec![vec![String::new()]]);
            max_cols = 1;
        }

        let mut warnings = Vec::new();

        // Pad short rows if needed
        if needs_padding {
            warnings.push(format!(
                "Padded rows with empty cells (max width: {} columns)",
                max_cols
            ));

            // Pad all rows to max_cols
            for chunk in chunks.iter_mut() {
                for row in chunk.iter_mut() {
                    if row.len() < max_cols {
                        row.resize(max_cols, String::new());
                    }
                }
            }
        }

        Ok(LoadResult {
            table: Table::from_chunks(chunks, max_cols),
            warnings,
        })
    }

    fn write_csv(&self, table: &Table) -> io::Result<()> {
        let path = self.file_path.as_ref().ok_or(io::ErrorKind::NotFound)?;
        let delim = self.format.and_then(|f| f.delimiter()).unwrap_or(b',');

        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        let mut csv_writer = csv::WriterBuilder::new()
            .delimiter(delim)
            .from_writer(writer);

        for row in table.rows_iter() {
            csv_writer
                .write_record(row)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }

        csv_writer
            .flush()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_format_detection() {
        assert_eq!(FileFormat::from_extension(&PathBuf::from("test.csv")), Some(FileFormat::Csv));
        assert_eq!(FileFormat::from_extension(&PathBuf::from("test.tsv")), Some(FileFormat::Tsv));
        assert_eq!(FileFormat::from_extension(&PathBuf::from("test.txt")), None);
    }

    #[test]
    fn test_csv_padding_warning() {
        let mut file = NamedTempFile::with_suffix(".csv").unwrap();
        writeln!(file, "a,b,c").unwrap();
        writeln!(file, "1,2").unwrap();  // Short row
        writeln!(file, "3,4,5").unwrap();

        let file_io = FileIO::new(Some(file.path().to_path_buf())).unwrap();
        let result = file_io.load_table().unwrap();

        assert_eq!(result.table.col_count(), 3);
        assert!(!result.warnings.is_empty());
        assert!(result.warnings[0].contains("Padded"));
    }
}
