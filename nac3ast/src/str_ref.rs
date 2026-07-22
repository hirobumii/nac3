use fxhash::FxBuildHasher;
use parking_lot::{Mutex, MutexGuard};
use std::{cell::RefCell, collections::HashMap, fmt, sync::LazyLock};
use string_interner::{DefaultBackend, StringInterner, symbol::SymbolU32};

pub type Interner = StringInterner<DefaultBackend, FxBuildHasher>;
static INTERNER: LazyLock<Mutex<Interner>> =
    LazyLock::new(|| Mutex::new(StringInterner::with_hasher(FxBuildHasher::default())));

thread_local! {
    static LOCAL_INTERNER: RefCell<HashMap<String, StrRef>> = RefCell::default();
}

#[derive(Eq, PartialEq, Copy, Clone, Hash)]
pub struct StrRef(SymbolU32);

impl fmt::Debug for StrRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s: String = (*self).into();
        write!(f, "{s:?}")
    }
}

impl fmt::Display for StrRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s: String = (*self).into();
        write!(f, "{s}")
    }
}

impl From<String> for StrRef {
    fn from(s: String) -> Self {
        get_str_ref(&mut get_str_ref_lock(), &s)
    }
}

impl From<&str> for StrRef {
    fn from(s: &str) -> Self {
        // thread local cache
        LOCAL_INTERNER.with(|local| {
            let mut local = local.borrow_mut();
            local.get(s).copied().unwrap_or_else(|| {
                let r = get_str_ref(&mut get_str_ref_lock(), s);
                local.insert(s.to_string(), r);
                r
            })
        })
    }
}

impl From<StrRef> for String {
    fn from(s: StrRef) -> Self {
        get_str_from_ref(&get_str_ref_lock(), s).to_string()
    }
}

pub fn get_str_ref_lock<'a>() -> MutexGuard<'a, Interner> {
    INTERNER.lock()
}

pub fn get_str_ref(lock: &mut MutexGuard<Interner>, str: &str) -> StrRef {
    StrRef(lock.get_or_intern(str))
}

#[must_use]
pub fn get_str_from_ref(lock: &Interner, id: StrRef) -> &str {
    lock.resolve(id.0).unwrap()
}

pub type Ident = StrRef;
