use std::io::{Read, Write};
use std::path::PathBuf;
use std::fs::{remove_file, rename, File};

use fxhash::{FxHashSet, FxHashMap};
use clap::Parser;
use scraper::Html;
use zip::ZipArchive;

use allsorts::binary::read::ReadScope;
use allsorts::font::read_cmap_subtable;
use allsorts::font_data::FontData;
use allsorts::subset::{CmapTarget, SubsetProfile};
use allsorts::tables::cmap::Cmap;
use allsorts::tables::{FontTableProvider};
use allsorts::{subset, tag};
use zip::write::{SimpleFileOptions};
use utils::{args_path_handle, open_file, zip_xhtml_read};

/// 用于快速根据内容将内 EPUB 字体子集化的小工具
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
    /// 要优化的 EPUB 文件路径（支持输入多个）
    ///
    /// 可传入 `.epub` 文件，或配合 `-w` 传入目录。
    #[arg(required = true)]
    files: Vec<PathBuf>
}


fn get_xhtml_words(string: &str) -> FxHashSet<char>
{
    Html::parse_document(string)
        .root_element()
        .text()
        .collect::<String>()
        .chars()
        .collect()
}


fn chars_to_glyphs<F: FontTableProvider>(
    font_provider: &F,
    text: &FxHashSet<char>,
) -> Vec<u16> {
    let cmap_data = font_provider.read_table_data(tag::CMAP).expect("读取CMAP表失败");
    let cmap = ReadScope::new(&cmap_data).read::<Cmap>().expect("解析CMAP表失败");
    let (_, cmap_subtable) = read_cmap_subtable(&cmap).expect("解析CMAP表失败").expect("解析CMAP表失败");

    let mut glyphs = Vec::new();

    for ch in text {
        let code_point = *ch as u32;
        let glyph_id = cmap_subtable.map_glyph(code_point).expect("转换为glyphid失败");
        if glyph_id.is_none() {
            continue;
        }
        glyphs.push(glyph_id.unwrap());
    };
    glyphs
}


fn main() {
    let args = Cli::parse();

    let epub_files = args_path_handle(args.files, false);

    for epub_file in epub_files {
        // 读取全部xhtml文件并解析得到需要的字符集
        let mut words: FxHashSet<char> = FxHashSet::default();
        let mut file = open_file(epub_file.file.clone());
        let xhtmls = zip_xhtml_read(&mut file);
        for i in xhtmls {
            words.extend(get_xhtml_words(i.as_str()));
        }
        println!("解析epub完毕");
        println!("字数：{}", words.len());

        let mut zip = ZipArchive::new(file).expect("读取EPUB文件失败");

        // 从zip里面得到全部字体文件的名字以备后续读取并处理
        let fonts = zip.file_names()
            .filter(|i| i.ends_with(".ttf") || i.ends_with(".otf") || i.ends_with(".woff"))
            .map(|i| i.to_string())
            .collect::<Vec<String>>();
        println!("字体文件数：{}", fonts.len());
        println!("字体列表：{:?}", fonts);
        println!("开始子集化字体文件");
        let mut font_data_map: FxHashMap<String, Vec<u8>> = FxHashMap::default();
        // 子集化主要逻辑
        for font_name in fonts {
            println!("正在处理字体文件：{}", font_name);
            let mut font_data = Vec::new();
            zip.by_name(&font_name).expect("读取字体文件失败").read_to_end(&mut font_data).expect("读取字体文件失败");

            let scope = ReadScope::new(&font_data);
            let font = scope.read::<FontData>().expect("解析字体文件失败");

            let provider = font.table_provider(1).expect("解析字体文件失败");

            let glyphs = chars_to_glyphs(&provider, &words);

            let sub_font = subset::subset(
                &provider,
                glyphs.as_slice(),
                &SubsetProfile::Minimal,
                CmapTarget::Unicode
            ).expect("子集化失败");

            font_data_map.insert(font_name, sub_font);
        }

        println!("子集化完毕");
        println!("开始写入新epub文件");
        // 创建zip文件
        let mut temp_zip = epub_file.file.clone();
        temp_zip.set_file_name(format!("{}.opt", epub_file.filename));
        let new_zip_file = File::create(&temp_zip).expect("创建新epub文件失败");
        let mut new_zip = zip::ZipWriter::new(new_zip_file);

        // 第一个写入mimetype
        new_zip.start_file(
            "mimetype",
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        ).expect("写入mimetype失败");
        std::io::copy(
            &mut zip.by_name("mimetype").expect("读取mimetype失败"),
            &mut new_zip).expect("复制mimetype失败"
        );

        for i in 0..zip.len() {
            let mut file = zip.by_index(i).expect("读取ZIP条目失败");
            let name = file.name().to_string();

            if name == "mimetype" {
                continue;
            }
            if font_data_map.contains_key(&name)
            {
                new_zip.start_file(
                    &name,
                    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
                ).expect("写入文件失败");
                new_zip.write_all(font_data_map.get(&name).unwrap()).expect("写入文件失败");
                continue;
            }

            new_zip.start_file(
                &name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
            ).expect("写入文件失败");
            std::io::copy(&mut file, &mut new_zip).expect("写入文件失败");
        }
        new_zip.finish().expect("写入文件失败");

        println!("写入完毕");
        println!("将删除旧文件并写入新文件");
        remove_file(&epub_file.file).expect("删除旧文件失败");
        rename(&temp_zip, &epub_file.file).expect("重命名文件失败");
        println!("处理完成");
    }
}
