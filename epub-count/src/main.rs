use std::path::{Path, PathBuf};
use std::process::exit;
use std::thread;
use std::thread::{JoinHandle};

use clap::Parser;
use scraper::Html;

use utils::*;

/// 一个用于统计 EPUB 文件字数的小工具
///
/// 支持直接指定文件，或通过 `-w` 递归遍历目录。
#[derive(Parser)]
#[command(
    version,
    about,
    long_about = None,
)]
struct Cli
{
    /// 要统计的 EPUB 文件路径（支持输入多个）
    ///
    /// 可传入 `.epub` 文件，或配合 `-w` 传入目录。
    #[arg(required = true)]
    files: Vec<PathBuf>,

    /// 递归遍历目录（walk directories）
    ///
    /// 当传入的是目录时，自动查找其中所有 `.epub` 文件并统计。
    #[arg(short ,long, default_value_t = false, action = clap::ArgAction::SetTrue)]
    walk: bool,


    /// 是否启用流式输出
    #[arg(short, long, default_value_t = false, action = clap::ArgAction::SetTrue)]
    stream_output: bool,


    /// 调整使用的线程数，默认为cpu线程数
    #[arg(short, long, default_value_t = get_cpu_count())]
    cpu_nums: usize
}

struct FileWordCount
{
    filename: String,
    word_count: u64
}


fn html_word_count(string: &String) -> u64
{
    Html::parse_document(string)
        .root_element()
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("")
        .chars()
        .count() as u64
}


fn get_epub_word_count<P: AsRef<Path>>(path: P) -> u64
{
    let file = open_file(path);
    let chars = zip_xhtml_read(file);
    let word_count: u64 = chars.iter().map(
        |s| html_word_count(s)
    ).sum::<u64>();

    word_count
}


fn main()
{
    let args = Cli::parse();

    let epub_files: Vec<FileData> = args_path_handle(args.files, args.walk);

    if epub_files.is_empty()
    {
        eprintln!("没有找到任何EPUB文件");
        exit(0)
    }

    let mut total_word_count: u64 = 0;
    let mut threads: Vec<JoinHandle<Vec<FileWordCount>>> = Vec::new();
    for files in split_vec(epub_files, args.cpu_nums)
    {
        threads.push(thread::spawn(move || {
            let mut infos: Vec<FileWordCount> = Vec::new();
            for f in files
            {
                let word_count = get_epub_word_count(f.file);
                let info = FileWordCount{
                    filename: f.filename,
                    word_count
                };
                if args.stream_output
                {
                    println!("{} 字数：{} 字", info.filename, info.word_count);
                }
                infos.push(info);
            }
            infos
        }))
    }

    if !args.stream_output
    {
        for handle in threads {
            let infos = handle.join().unwrap();
            for info in infos {
                println!("{} 字数：{} 字", info.filename, info.word_count);
                total_word_count += info.word_count;
            }
        }
    }
    else
    {
        for handle in threads
        {
            let infos = handle.join().unwrap();
            total_word_count += infos.iter().map(|info| info.word_count).sum::<u64>();
        }
    }

    println!("总字数：{} 字", total_word_count)
}