use inkwell::llvm_sys::prelude::LLVMTypeRef;
use inkwell::types::{AsTypeRef, BasicTypeEnum, PointerType};


#[derive(Debug)]
pub struct OpaquePointerType<'ctx> {
    pub ptr_ty: PointerType<'ctx>,
    pub inner_ty: Box<Option<ExtendedTypeEnum<'ctx>>>,
}

#[derive(Debug)]
pub enum ExtendedTypeEnum<'ctx> {
    BasicEnum(BasicTypeEnum<'ctx>),
    OpaquePointer(OpaquePointerType<'ctx>),
}

unsafe impl AsTypeRef for ExtendedTypeEnum<'_> {
    fn as_type_ref(&self) -> LLVMTypeRef {
        match *self {
            ExtendedTypeEnum::OpaquePointer(_) => panic!("Opaque Pointer Reference is not allowed"),
            ExtendedTypeEnum::BasicEnum(t) => t.as_type_ref(),
        }
    }
}

impl ExtendedTypeEnum<'_> {
    pub fn get_type(&self) -> BasicTypeEnum<'_> {
        match self {
            ExtendedTypeEnum::BasicEnum(t) => t.clone(),
            ExtendedTypeEnum::OpaquePointer(t) => t.ptr_ty.clone().into(),
        }
    }
}

impl<'ctx> From<OpaquePointerType<'ctx>> for ExtendedTypeEnum<'ctx> {
    fn from(value: OpaquePointerType) -> ExtendedTypeEnum {
        ExtendedTypeEnum::OpaquePointer(value)
    }
}
impl<'ctx> From<BasicTypeEnum<'ctx>> for ExtendedTypeEnum<'ctx> {
    fn from(value: BasicTypeEnum) -> ExtendedTypeEnum {
        ExtendedTypeEnum::BasicEnum(value)
    }
}

impl<'ctx> TryFrom<ExtendedTypeEnum<'ctx>> for OpaquePointerType<'ctx> {
    type Error = ();

    fn try_from(value: ExtendedTypeEnum<'ctx>) -> Result<Self, Self::Error> {
        match value {
            ExtendedTypeEnum::OpaquePointer(ty) => Ok(ty),
            _ => Err(()),
        }
    }
}
impl<'ctx> TryFrom<ExtendedTypeEnum<'ctx>> for BasicTypeEnum<'ctx> {
    type Error = ();

    fn try_from(value: ExtendedTypeEnum<'ctx>) -> Result<Self, Self::Error> {
        match value {
            ExtendedTypeEnum::BasicEnum(ty) => Ok(ty),
            _ => Err(()),
        }
    }
}


impl<'ctx> ExtendedTypeEnum<'ctx> {
    pub fn into_basic_type(self) -> BasicTypeEnum<'ctx> {
        if let ExtendedTypeEnum::BasicEnum(t) = self {
            t
        } else {
            panic!("Found {:?} but expected the ArrayType variant", self);
        }
    }

    pub fn into_opaque_pointer(self) -> OpaquePointerType<'ctx> {
        if let ExtendedTypeEnum::OpaquePointer(t) = self {
            t
        } else {
            panic!("Found {:?} but expected the ArrayType variant", self);
        }
    }

    pub fn is_basic_enum(self) -> bool {
        matches!(self, ExtendedTypeEnum::BasicEnum(_))
    }

    pub fn is_opaque_pointer(self) -> bool {
        matches!(self, ExtendedTypeEnum::OpaquePointer(_))
    }
}