use std::io::{self, Read};
use std::path::Path;

pub fn read(path: &Path, remaining: u64) -> io::Result<Option<Vec<u8>>> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "attachment path must be absolute",
        ));
    }
    let mut file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "attachment must be a regular file",
        ));
    }
    read_bytes(
        &mut file,
        metadata.len(),
        remaining.min(mail_core::MAX_ATTACHMENTS_BYTES),
    )
}

fn read_bytes(reader: &mut impl Read, size: u64, remaining: u64) -> io::Result<Option<Vec<u8>>> {
    if size > remaining {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    // Metadata is only an early refusal; this also bounds files that grow after it.
    reader
        .take(remaining.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= remaining).then_some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Source {
        left: usize,
        admitted: usize,
    }
    impl Read for Source {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            let count = self.left.min(output.len());
            output[..count].fill(17);
            self.left -= count;
            self.admitted += count;
            Ok(count)
        }
    }

    #[test]
    fn declared_oversize_does_not_read_file_contents() {
        let mut source = Source {
            left: 257,
            admitted: 0,
        };
        assert!(read_bytes(&mut source, 257, 128).unwrap().is_none());
        assert_eq!(source.admitted, 0);
    }

    #[test]
    fn growth_after_metadata_is_stopped_at_one_extra_byte() {
        for hint in [0, 1, 128] {
            let mut source = Source {
                left: 257,
                admitted: 0,
            };
            assert!(read_bytes(&mut source, hint, 128).unwrap().is_none());
            assert_eq!(source.admitted, 129);
        }
    }

    #[test]
    fn exact_budget_and_empty_file_preserve_all_bytes() {
        for limit in [0, 128] {
            let mut source = Source {
                left: limit,
                admitted: 0,
            };
            assert_eq!(
                read_bytes(&mut source, limit as u64, limit as u64).unwrap(),
                Some(vec![17; limit])
            );
        }
    }
}
