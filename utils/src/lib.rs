use std::fs::OpenOptions;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use std::thread::available_parallelism;
use walkdir::{DirEntry, WalkDir};
use zip::ZipArchive;
use memmap2::Mmap;

pub fn get_all_epub_walkdir<P: AsRef<Path>>(path: P) -> Vec<PathBuf> {
    fn is_epub(entry: &DirEntry) -> bool {
        entry.file_type().is_file()
            && entry
            .path()
            .extension()
            .map_or(false, |ext| ext.eq_ignore_ascii_case("epub"))
    }

    WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| {
            e.file_type().is_dir() || is_epub(e)
        })
        .filter_map(|e| e.ok())
        .filter(is_epub)
        .map(|e| e.into_path())
        .collect()
}

pub fn zip_xhtml_read<W: Read + Seek>(file: W) -> Vec<String> {
    let mut zip = ZipArchive::new(file).expect("读取zip文件时出现错误");

    let n = zip.len();
    let mut results = Vec::new();

    for i in 0..n {
        let mut file = zip.by_index(i).expect("遍历zip文件列表时出现错误");
        let name = file.name();

        if !(name.ends_with(".xhtml") || name.ends_with(".html")) {
            continue;
        }
        if name == "toc.xhtml" || name == "toc.html" {
            continue;
        }

        let size = file.size();
        let mut content = String::with_capacity(size as usize);

        file.read_to_string(&mut content).expect("读取xhtml文件时出现错误");
        results.push(content);
    }

    results
}


pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

pub fn open_file<P: AsRef<Path>>(p: P) -> Box<dyn ReadSeek>
{
    let file = OpenOptions::new()
        .read(true)
        .write(false)
        .create(false)
        .open(p)
        .expect("打开文件失败");
    let file_mmap = unsafe { Mmap::map(&file) };
    match file_mmap {
        Ok(mmap) => Box::new(Cursor::new(mmap)),
        Err(_) => {
            Box::new(file)
        }
    }
}


pub fn split_vec<T>(mut vec: Vec<T>, n: usize) -> Vec<Vec<T>> {
    if n == 0 || vec.is_empty() {
        return vec![vec];
    }

    let len = vec.len();
    let chunk_size = (len + n - 1) / n;
    let mut result = Vec::new();

    while !vec.is_empty() {
        let take = chunk_size.min(vec.len());
        let chunk = vec.drain(..take).collect::<Vec<T>>();
        result.push(chunk);
    }

    result
}

pub fn get_cpu_count() -> usize {
    available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}


pub struct FileData
{
    pub filename: String,
    pub file: PathBuf
}

pub fn args_path_handle(paths: Vec<PathBuf>, walk: bool) -> Vec<FileData>
{
    let mut epub_files: Vec<FileData> = Vec::new();
    for path in paths {

        if !path.exists() {
            eprintln!("文件/目录 {} 不存在", path.clone().display());
            continue;
        }

        if walk && path.is_dir()
        {
            for p in get_all_epub_walkdir(path.clone()) {
                let s = FileData {
                    filename: p.file_name().unwrap().to_str().unwrap().to_string(),
                    file: p
                };
                epub_files.push(s);
            }
        }
        else if !walk && path.is_dir()
        {
            continue;
        }
        else if path.is_file()
        {
            let s = FileData {
                filename: path.file_name().unwrap().to_str().unwrap().to_string(),
                file: path
            };
            epub_files.push(s);
        }
        else
        {
            panic!("未知输入")
        }
    }
    epub_files
}
