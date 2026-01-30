use std::path::{PathBuf, Path};
use std::fs::File;
use std::io::{self, BufReader, BufWriter};

use crate::table::Table;

pub struct FileIO {
    pub file_path: Option<PathBuf>
}

impl FileIO {
    pub fn new(file_path: Option<PathBuf>) -> io::Result<Self> {
        Ok(Self {file_path} )
    }

    pub fn read_csv(&mut self) -> io::Result<Table> {
        let delim = self.get_delim();
        let path = self.file_path.as_ref().ok_or(io::ErrorKind::NotFound)?;
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut csv_reader = csv::ReaderBuilder::new()
            .delimiter(delim)
            .has_headers(false)
            .flexible(true)
            .from_reader(reader);

        let mut cells: Vec<Vec<String>> = Vec::new();

        for result in csv_reader.records() {
            let record = result.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let row: Vec<String> = record.iter().map(|s| s.to_string()).collect();
            cells.push(row);
        }

        if cells.is_empty() {
            cells.push(vec![String::new()]);
        }

        // find the maximum length and pad
        let max_len = cells.iter().map(|x| x.len()).max();

        for row in cells.iter_mut() {
            row.resize(max_len.unwrap_or(0), String::new());
        }

        Ok(Table::new(cells))
    }

    pub fn save_csv(&mut self, table: &mut Table) -> io::Result<()> {
        let delim = self.get_delim();
        let path = self.file_path.as_ref().ok_or(io::ErrorKind::NotFound)?;

        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        let mut csv_writer = csv::WriterBuilder::new()
            .delimiter(delim)
            .from_writer(writer);

        for row in &table.cells {
            csv_writer
                .write_record(row)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }

        csv_writer
            .flush()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(())
    }

    pub fn load_table(&mut self) -> io::Result<Table> {
        if let None = self.file_path {
            return Ok(Table::new(vec![vec![String::new()]]));
        }
        
        return self.read_csv()
    }

    pub fn file_name(&mut self) -> String {
        if let Some(ref path) = self.file_path {
            return path.display().to_string();
        }

        String::from("")
    }

    pub fn get_delim(&mut self) -> u8 {
         let ext = self.file_path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str());
        
         if ext == Some("tsv") {
             return b'\t';
         }
         b','
    }

    pub fn write(&mut self, table: &mut Table) -> io::Result<()> {
        if let None = self.file_path {
            return Err(io::Error::new(io::ErrorKind::NotFound, "file does not exist"));
        }
        return self.save_csv(table)
    }
}
