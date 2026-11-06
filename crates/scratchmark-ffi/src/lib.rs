//! 实验性 C ABI；调用方内存义务参 include/scratchmark.h 与 spec/M0.md。
mod buffers;
mod exports;
use scratchmark_core::{Document, Error, Result};
use std::{
    collections::BTreeMap,
    panic::catch_unwind,
    sync::{Arc, Mutex, OnceLock},
};

type Engine = Arc<Mutex<Document>>;
struct Registry {
    next: u64,
    engines: BTreeMap<u64, Engine>,
}
static ENGINES: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> &'static Mutex<Registry> {
    ENGINES.get_or_init(|| {
        Mutex::new(Registry {
            next: 1,
            engines: BTreeMap::new(),
        })
    })
}

fn create() -> Result<u64> {
    let mut r = registry().lock().map_err(|_| Error::Internal)?;
    let id = r.next;
    let next = id.checked_add(1).ok_or(Error::LimitExceeded)?;
    r.engines
        .insert(id, Arc::new(Mutex::new(Document::default())));
    r.next = next;
    Ok(id)
}

fn destroy(handle: u64) -> Result<()> {
    registry()
        .lock()
        .map_err(|_| Error::Internal)?
        .engines
        .remove(&handle)
        .ok_or(Error::InvalidHandle)?;
    Ok(())
}

fn with_doc<T>(handle: u64, operation: impl FnOnce(&mut Document) -> Result<T>) -> Result<T> {
    // 注册表只保护查找，不把不同文档的计算串行化。
    let engine = registry()
        .lock()
        .map_err(|_| Error::Internal)?
        .engines
        .get(&handle)
        .cloned()
        .ok_or(Error::InvalidHandle)?;
    let mut document = engine.lock().map_err(|_| Error::Internal)?;
    operation(&mut document)
}

fn guard(operation: impl FnOnce() -> Result<()> + std::panic::UnwindSafe) -> u32 {
    match catch_unwind(operation) {
        Ok(Ok(())) => 0,
        Ok(Err(e)) => e as u32,
        Err(payload) => {
            // 自定义 panic payload 的析构也可能再次 panic，ABI 边界不能执行它。
            std::mem::forget(payload);
            Error::Internal as u32
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn sm_abi_version() -> u32 {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn sm_features() -> u64 {
    // bit 0：M2 派生笔刷；bit 1：M3 页面标识与视口变换；bit 2：M4 命中与整笔擦除。
    0b111
}

#[unsafe(no_mangle)]
pub extern "C" fn sm_destroy(handle: u64) -> u32 {
    guard(|| destroy(handle))
}

#[unsafe(no_mangle)]
pub extern "C" fn sm_begin(handle: u64, width: f64, color: u32) -> u32 {
    guard(|| with_doc(handle, |d| d.begin(width, color)))
}

macro_rules! operation {
    ($symbol:ident, $method:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $symbol(handle: u64) -> u32 {
            guard(|| with_doc(handle, |d| d.$method()))
        }
    };
}
operation!(sm_finish, finish);
operation!(sm_cancel, cancel);
operation!(sm_undo, undo);
operation!(sm_redo, redo);
