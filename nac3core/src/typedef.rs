use std::collections::HashMap;
use std::rc::Rc;

#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub struct PrimitiveId(pub(crate) usize);

#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub struct ClassId(pub(crate) usize);

#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub struct ParamId(pub(crate) usize);

#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub struct VariableId(pub(crate) usize);

#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub enum Type {
    BotType,
    SelfType,
    PrimitiveType(PrimitiveId),
    ClassType(ClassId),
    VirtualClassType(ClassId),
    ParametricType(ParamId, Vec<Rc<Type>>),
    TypeVariable(VariableId),
}

#[derive(Clone)]
pub struct FnDef {
    // we assume methods first argument to be SelfType,
    // so the first argument is not contained here
    pub args: Vec<Rc<Type>>,
    pub result: Option<Rc<Type>>,
}

#[derive(Clone)]
pub struct TypeDef<'a> {
    pub name: &'a str,
    pub fields: HashMap<&'a str, Rc<Type>>,
    pub methods: HashMap<&'a str, FnDef>,
}

#[derive(Clone)]
pub struct ClassDef<'a> {
    pub base: TypeDef<'a>,
    pub parents: Vec<ClassId>,
}

#[derive(Clone)]
pub struct ParametricDef<'a> {
    pub base: TypeDef<'a>,
    pub params: Vec<VariableId>,
}

#[derive(Clone)]
pub struct VarDef<'a> {
    pub name: &'a str,
    pub bound: Vec<Rc<Type>>,
}

pub struct GlobalContext<'a> {
    primitive_defs: Vec<TypeDef<'a>>,
    class_defs: Vec<ClassDef<'a>>,
    parametric_defs: Vec<ParametricDef<'a>>,
    var_defs: Vec<VarDef<'a>>,
    sym_table: HashMap<&'a str, Type>,
    fn_table: HashMap<&'a str, FnDef>,
}

impl<'a> GlobalContext<'a> {
    pub fn new(primitives: Vec<TypeDef<'a>>) -> GlobalContext {
        let mut sym_table = HashMap::new();
        for (i, t) in primitives.iter().enumerate() {
            sym_table.insert(t.name, Type::PrimitiveType(PrimitiveId(i)));
        }
        return GlobalContext {
            primitive_defs: primitives,
            class_defs: Vec::new(),
            parametric_defs: Vec::new(),
            var_defs: Vec::new(),
            fn_table: HashMap::new(),
            sym_table,
        };
    }

    pub fn add_class(&mut self, def: ClassDef<'a>) -> ClassId {
        self.sym_table.insert(
            def.base.name,
            Type::ClassType(ClassId(self.class_defs.len())),
        );
        self.class_defs.push(def);
        ClassId(self.class_defs.len() - 1)
    }

    pub fn add_parametric(&mut self, def: ParametricDef<'a>) -> ParamId {
        let params = def
            .params
            .iter()
            .map(|&v| Rc::new(Type::TypeVariable(v)))
            .collect();
        self.sym_table.insert(
            def.base.name,
            Type::ParametricType(ParamId(self.parametric_defs.len()), params),
        );
        self.parametric_defs.push(def);
        ParamId(self.parametric_defs.len() - 1)
    }

    pub fn add_variable(&mut self, def: VarDef<'a>) -> VariableId {
        self.sym_table.insert(
            def.name,
            Type::TypeVariable(VariableId(self.var_defs.len())),
        );
        self.add_variable_private(def)
    }

    pub fn add_variable_private(&mut self, def: VarDef<'a>) -> VariableId {
        self.var_defs.push(def);
        VariableId(self.var_defs.len() - 1)
    }

    pub fn add_fn(&mut self, name: &'a str, def: FnDef) {
        self.fn_table.insert(name, def);
    }

    pub fn get_fn(&self, name: &str) -> Option<&FnDef> {
        self.fn_table.get(name)
    }

    pub fn get_primitive_mut(&mut self, id: PrimitiveId) -> &mut TypeDef<'a> {
        self.primitive_defs.get_mut(id.0).unwrap()
    }

    pub fn get_primitive(&self, id: PrimitiveId) -> &TypeDef {
        self.primitive_defs.get(id.0).unwrap()
    }

    pub fn get_class_mut(&mut self, id: ClassId) -> &mut ClassDef<'a> {
        self.class_defs.get_mut(id.0).unwrap()
    }

    pub fn get_class(&self, id: ClassId) -> &ClassDef {
        self.class_defs.get(id.0).unwrap()
    }

    pub fn get_parametric_mut(&mut self, id: ParamId) -> &mut ParametricDef<'a> {
        self.parametric_defs.get_mut(id.0).unwrap()
    }

    pub fn get_parametric(&self, id: ParamId) -> &ParametricDef {
        self.parametric_defs.get(id.0).unwrap()
    }

    pub fn get_variable_mut(&mut self, id: VariableId) -> &mut VarDef<'a> {
        self.var_defs.get_mut(id.0).unwrap()
    }

    pub fn get_variable(&self, id: VariableId) -> &VarDef {
        self.var_defs.get(id.0).unwrap()
    }

    pub fn get_type(&self, name: &str) -> Option<Type> {
        // TODO: change this to handle import
        self.sym_table.get(name).map(|v| v.clone())
    }
}

impl Type {
    pub fn subst(&self, map: &HashMap<VariableId, Rc<Type>>) -> Type {
        match self {
            Type::TypeVariable(id) => map.get(id).map(|v| v.as_ref()).unwrap_or(self).clone(),
            Type::ParametricType(id, params) => Type::ParametricType(
                *id,
                params
                    .iter()
                    .map(|v| v.as_ref().subst(map).into())
                    .collect(),
            ),
            _ => self.clone(),
        }
    }

    pub fn inv_subst(&self, map: &[(Rc<Type>, Rc<Type>)]) -> Rc<Type> {
        for (from, to) in map.iter() {
            if self == from.as_ref() {
                return to.clone();
            }
        }
        match self {
            Type::ParametricType(id, params) => Type::ParametricType(
                *id,
                params
                    .iter()
                    .map(|v| v.as_ref().inv_subst(map).into())
                    .collect(),
            ),
            _ => self.clone(),
        }
        .into()
    }

    pub fn get_subst(&self, ctx: &GlobalContext) -> HashMap<VariableId, Rc<Type>> {
        match self {
            Type::ParametricType(id, params) => {
                let vars = &ctx.get_parametric(*id).params;
                vars.iter()
                    .zip(params)
                    .map(|(v, p)| (*v, p.as_ref().clone().into()))
                    .collect()
            }
            // if this proves to be slow, we can use option type
            _ => HashMap::new(),
        }
    }

    pub fn get_base<'b: 'a, 'a>(&'a self, ctx: &'b GlobalContext) -> Option<&'b TypeDef> {
        match self {
            Type::PrimitiveType(id) => Some(ctx.get_primitive(*id)),
            Type::ClassType(id) | Type::VirtualClassType(id) => Some(&ctx.get_class(*id).base),
            Type::ParametricType(id, _) => Some(&ctx.get_parametric(*id).base),
            _ => None,
        }
    }
}
