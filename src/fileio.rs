use std::path::{PathBuf, Path};
use std::fs;
use std::ffi::OsStr;
use std::io::{self, BufRead, BufReader, BufWriter};

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
}

/// Common delimiters to detect
const CANDIDATE_DELIMITERS: &[u8] = &[b',', b'\t', b';', b'|'];

/// Detect the most likely delimiter by analyzing the first N lines
fn detect_delimiter(path: &PathBuf, sample_lines: usize) -> Option<u8> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut counts: Vec<Vec<usize>> = vec![Vec::new(); CANDIDATE_DELIMITERS.len()];

    for (line_idx, line_result) in reader.lines().enumerate() {
        if line_idx >= sample_lines {
            break;
        }
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        // Count occurrences of each delimiter in this line
        for (delim_idx, &delim) in CANDIDATE_DELIMITERS.iter().enumerate() {
            let count = line.bytes().filter(|&b| b == delim).count();
            counts[delim_idx].push(count);
        }
    }

    // Find the delimiter with the most consistent non-zero count
    let mut best_delim: Option<u8> = None;
    let mut best_score: f64 = 0.0;

    for (delim_idx, line_counts) in counts.iter().enumerate() {
        if line_counts.is_empty() {
            continue;
        }

        // Calculate consistency: we want delimiters that appear the same number of times
        // on each line (low variance) and appear at least once (non-zero mean)
        let sum: usize = line_counts.iter().sum();
        let mean = sum as f64 / line_counts.len() as f64;

        if mean < 1.0 {
            // Delimiter doesn't appear consistently
            continue;
        }

        // Calculate variance
        let variance: f64 = line_counts.iter()
            .map(|&c| (c as f64 - mean).powi(2))
            .sum::<f64>() / line_counts.len() as f64;

        // Score: higher mean and lower variance is better
        // Use coefficient of variation (lower is more consistent)
        let cv = if mean > 0.0 { variance.sqrt() / mean } else { f64::MAX };
        let score = mean / (1.0 + cv);

        if score > best_score {
            best_score = score;
            best_delim = Some(CANDIDATE_DELIMITERS[delim_idx]);
        }
    }

    best_delim
}

/// determine the filename to write the fork() output to
pub fn next_fork_filename_suffix_wins(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or(Path::new("."));
    let file_name = path.file_name().unwrap_or(OsStr::new("tabular_fork.csv")).to_string_lossy();

    let (stem, ext) = if let Some(s) = file_name.strip_suffix(".csv") {
        (s, "csv")
    } else if let Some(s) = file_name.strip_suffix(".tsv") {
        (s, "tsv")
    } else {
        panic!("expected .csv or .tsv filename");
    };

    // Extract (header, start_n)
    let (header, start_n) = match stem.rsplit_once('.') {
        Some((h, n)) if n.chars().all(|c| c.is_ascii_digit()) => {
            (h.to_string(), n.parse::<usize>().unwrap())
        }
        _ => (stem.to_string(), 0),
    };

    // Find maximum suffix among:
    // - `header.csv` => suffix 0
    // - `header.K.csv` => suffix K
    let mut max_suffix = start_n;

    for entry in fs::read_dir(parent).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().to_string();

        let Some(stem2) = name.strip_suffix(ext) else { continue; };

        if stem2 == header {
            max_suffix = max_suffix.max(0);
            continue;
        }

        let Some((h, n)) = stem2.rsplit_once('.') else { continue; };
        if h != header { continue; }
        if !n.chars().all(|c| c.is_ascii_digit()) { continue; }

        if let Ok(k) = n.parse::<usize>() {
            max_suffix = max_suffix.max(k);
        }
    }

    parent.join(format!("{}.{}.{}", header, max_suffix + 1, ext))
}

/// Result of loading a file, including any warnings
pub struct LoadResult {
    pub table: Table,
    pub warnings: Vec<String>,
}

pub struct FileIO {
    pub file_path: Option<PathBuf>,
    format: Option<FileFormat>,
    delimiter: u8,
    max_dim: (usize, usize),
    read_only: bool
}

impl FileIO {
    /// Create a new FileIO with optional delimiter override
    /// If delimiter is None, auto-detect from file content (or fall back to extension/comma)
    pub fn new(file_path: Option<PathBuf>, delimiter: Option<u8>, read_only: bool) -> io::Result<Self> {
        let format = file_path.as_ref().and_then(FileFormat::from_extension);

        // Determine delimiter: explicit > detected > extension-based > comma default
        let delimiter = if let Some(d) = delimiter {
            d
        } else if let Some(ref path) = file_path {
            if path.exists() {
                // Auto-detect from file content
                detect_delimiter(path, 30).unwrap_or_else(|| {
                    // Fall back to extension-based
                    match format {
                        Some(FileFormat::Tsv) => b'\t',
                        _ => b',',
                    }
                })
            } else {
                // New file - use extension or default
                match format {
                    Some(FileFormat::Tsv) => b'\t',
                    _ => b',',
                }
            }
        } else {
            b','
        };

        let max_dim = (50000000, 50000000);
        Ok(Self { file_path, format, delimiter, max_dim, read_only })
    }

    pub fn fork(&self) -> FileIO {
        let default_fname = match self.delimiter {
            b',' => "tabular_fork.csv",
            b'\t' => "tabular_fork.tsv",
            _  => "tabular_fork.csv"
        };
        let fpath = self.file_path.clone().unwrap_or_else(|| PathBuf::from(default_fname));
        FileIO {
            file_path: Some(next_fork_filename_suffix_wins(&fpath)),
            format: self.format,
            delimiter: self.delimiter,
            max_dim: self.max_dim,
            read_only: false
        }
    }

    /// Get the detected/configured delimiter
    pub fn delimiter(&self) -> u8 {
        self.delimiter
    }

    /// Get a human-readable name for the delimiter
    pub fn delimiter_name(&self) -> &'static str {
        match self.delimiter {
            b',' => "comma",
            b'\t' => "tab",
            b';' => "semicolon",
            b'|' => "pipe",
            _ => "custom",
        }
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
        let delim = self.delimiter;

        // Check if file exists - if not, create empty table
        if !path.exists() {
            return Ok(LoadResult {
                table: Table::new(vec![vec![String::new(); 5]; 10]),
                warnings: vec![format!("New file: {}", path.display())],
            });
        }

        let file = fs::File::open(path)?;
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
        if self.read_only {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "file opened in read-only mode (use ':fork' to save your work)"));
        }
        let path = self.file_path.as_ref().ok_or(io::ErrorKind::NotFound)?;
        let delim = self.delimiter;

        let file = fs::File::create(path)?;
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

        let file_io = FileIO::new(Some(file.path().to_path_buf()), None).unwrap();
        let result = file_io.load_table().unwrap();

        assert_eq!(result.table.col_count(), 3);
        assert!(!result.warnings.is_empty());
        assert!(result.warnings[0].contains("Padded"));
    }
}
