//! Creation of ar archives like for the lib and staticlib crate type

use std::fs::File;
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};

use llvm_archive_writer::{
    get_native_object_symbols, write_archive_to_stream, ArchiveKind, NewArchiveMember,
};
use rustc_codegen_ssa::back::archive::ArchiveBuilder;
use rustc_session::Session;

use object::read::archive::ArchiveFile;
use object::ReadCache;

#[derive(Debug)]
enum ArchiveEntry {
    FromArchive { archive_index: usize, file_range: (u64, u64) },
    File(PathBuf),
}

pub(crate) struct ArArchiveBuilder<'a> {
    sess: &'a Session,
    dst: PathBuf,
    archive_kind: ArchiveKind,

    src_archives: Vec<File>,
    // Don't use `HashMap` here, as the order is important. `rust.metadata.bin` must always be at
    // the end of an archive for linkers to not get confused.
    entries: Vec<(Vec<u8>, ArchiveEntry)>,
}

impl<'a> ArchiveBuilder<'a> for ArArchiveBuilder<'a> {
    fn new(sess: &'a Session, output: &Path, input: Option<&Path>) -> Self {
        let (src_archives, entries) = if let Some(input) = input {
            let read_cache = ReadCache::new(File::open(input).unwrap());
            let archive = ArchiveFile::parse(&read_cache).unwrap();
            let mut entries = Vec::new();

            for entry in archive.members() {
                let entry = entry.unwrap();
                entries.push((
                    entry.name().to_vec(),
                    ArchiveEntry::FromArchive { archive_index: 0, file_range: entry.file_range() },
                ));
            }

            (vec![read_cache.into_inner()], entries)
        } else {
            (vec![], Vec::new())
        };

        ArArchiveBuilder {
            sess,
            dst: output.to_path_buf(),
            archive_kind: match &*sess.target.archive_format {
                "gnu" => ArchiveKind::Gnu,
                "darwin" => ArchiveKind::Darwin,
                _ => panic!(),
            },

            src_archives,
            entries,
        }
    }

    fn src_files(&mut self) -> Vec<String> {
        self.entries.iter().map(|(name, _)| String::from_utf8(name.clone()).unwrap()).collect()
    }

    fn remove_file(&mut self, name: &str) {
        let index = self
            .entries
            .iter()
            .position(|(entry_name, _)| entry_name == name.as_bytes())
            .expect("Tried to remove file not existing in src archive");
        self.entries.remove(index);
    }

    fn add_file(&mut self, file: &Path) {
        self.entries.push((
            file.file_name().unwrap().to_str().unwrap().to_string().into_bytes(),
            ArchiveEntry::File(file.to_owned()),
        ));
    }

    fn add_archive<F>(&mut self, archive_path: &Path, mut skip: F) -> std::io::Result<()>
    where
        F: FnMut(&str) -> bool + 'static,
    {
        let read_cache = ReadCache::new(std::fs::File::open(&archive_path)?);
        let archive = ArchiveFile::parse(&read_cache)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let archive_index = self.src_archives.len();

        for entry in archive.members() {
            let entry = entry.map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            let file_name = String::from_utf8(entry.name().to_vec())
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            if !skip(&file_name) {
                self.entries.push((
                    file_name.into_bytes(),
                    ArchiveEntry::FromArchive { archive_index, file_range: entry.file_range() },
                ));
            }
        }

        self.src_archives.push(read_cache.into_inner());
        Ok(())
    }

    fn build(mut self) {
        let mut entries = Vec::new();

        for (entry_name, entry) in self.entries {
            // FIXME only read the symbol table of the object files to avoid having to keep all
            // object files in memory at once, or read them twice.
            let data = match entry {
                ArchiveEntry::FromArchive { archive_index, file_range } => {
                    // FIXME read symbols from symtab
                    let src_read_cache = &mut self.src_archives[archive_index];

                    src_read_cache.seek(io::SeekFrom::Start(file_range.0)).unwrap();
                    let mut data = std::vec::from_elem(0, usize::try_from(file_range.1).unwrap());
                    src_read_cache.read_exact(&mut data).unwrap();

                    data
                }
                ArchiveEntry::File(file) => std::fs::read(file).unwrap_or_else(|err| {
                    self.sess.fatal(&format!(
                        "error while reading object file during archive building: {}",
                        err
                    ));
                }),
            };

            entries.push(NewArchiveMember {
                buf: Box::new(data),
                get_symbols: get_native_object_symbols,
                member_name: String::from_utf8(entry_name).expect("FIXME"),
                mtime: 0,
                uid: 0,
                gid: 0,
                perms: 0o644,
            })
        }

        let mut w = File::create(&self.dst).unwrap_or_else(|err| {
            self.sess.fatal(&format!("error opening destination during archive building: {}", err));
        });

        write_archive_to_stream(&mut w, &entries, true, self.archive_kind, true, false)
            .expect("FIXME");
    }

    fn inject_dll_import_lib(
        &mut self,
        _lib_name: &str,
        _dll_imports: &[rustc_session::cstore::DllImport],
        _tmpdir: &rustc_data_structures::temp_dir::MaybeTempDir,
    ) {
        bug!("injecting dll imports is not supported");
    }
}
