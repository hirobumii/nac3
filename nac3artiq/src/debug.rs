use itertools::Itertools as _;
use nac3core::{toplevel::TopLevelDef, typecheck::typedef::Unifier};

use crate::symbol_resolver::{InnerResolver, PyValueHandle};

impl InnerResolver {
    pub fn debug_str(&self, tld: Option<&[TopLevelDef]>, unifier: &Option<&mut Unifier>) -> String {
        fn fmt_elems(elems: &str) -> String {
            if elems.is_empty() { String::new() } else { format!("\n{elems}\n\t") }
        }
        fn stringify_pyvalue_handle(handle: &PyValueHandle) -> String {
            format!("(id: {}, value: {})", handle.0, handle.1)
        }
        fn stringify_tld(tld: &TopLevelDef) -> String {
            match tld {
                TopLevelDef::Module { name, .. } => {
                    format!("TopLevelDef::Module {{ name: {name} }}")
                }
                TopLevelDef::Class { name, .. } => {
                    format!("TopLevelDef::Class {{ name: {name} }}")
                }
                TopLevelDef::Function { name, .. } => {
                    format!("TopLevelDef::Function {{ name: {name} }}")
                }
            }
        }

        let mut str = String::new();
        str.push_str("nac3artiq::InnerResolver {");

        str.push_str(
            format!(
                "\n\tid_to_type: {{{}}},",
                fmt_elems(
                    self.id_to_type
                        .read()
                        .iter()
                        .sorted_by_cached_key(|(k, _)| k.to_string())
                        .map(|(k, v)| {
                            let ty_str = unifier
                                .as_ref()
                                .map_or_else(|| format!("{v:?}"), |unifier| unifier.stringify(*v));
                            format!("\t\t{k} -> {ty_str}")
                        })
                        .join(",\n")
                        .as_str()
                ),
            )
            .as_str(),
        );

        str.push_str(
            format!(
                "\n\tid_to_def: {{{}}},",
                fmt_elems(
                    self.id_to_def
                        .read()
                        .iter()
                        .sorted_by_cached_key(|(k, _)| k.to_string())
                        .map(|(k, v)| {
                            let tld_str = tld
                                .map_or_else(|| format!("{v:?}"), |tlds| stringify_tld(&tlds[v.0]));
                            format!("\t\t{k} -> {tld_str}")
                        })
                        .join(",\n")
                        .as_str()
                )
            )
            .as_str(),
        );

        str.push_str(
            format!(
                "\n\tid_to_pyval: {{{}}},",
                fmt_elems(
                    self.id_to_pyval
                        .read()
                        .iter()
                        .sorted_by_cached_key(|(k, _)| k.to_string())
                        .map(|(k, v)| { format!("\t\t{k} -> {}", stringify_pyvalue_handle(v)) })
                        .join(",\n")
                        .as_str()
                )
            )
            .as_str(),
        );

        str.push_str(
            format!(
                "\n\tid_to_primitive: {{{}}},",
                fmt_elems(
                    self.id_to_primitive
                        .read()
                        .iter()
                        .sorted_by_key(|(k, _)| *k)
                        .map(|(k, v)| { format!("\t\t{k} -> {v:?}") })
                        .join(",\n")
                        .as_str()
                )
            )
            .as_str(),
        );

        str.push_str(
            format!(
                "\n\tfield_to_val: {{{}}},",
                fmt_elems(
                    self.field_to_val
                        .read()
                        .iter()
                        .sorted_by_key(|((id, _), _)| *id)
                        .map(|((id, name), pyval)| {
                            format!(
                                "\t\t({id}, {name}) -> {}",
                                pyval.as_ref().map_or_else(
                                    || String::from("None"),
                                    |pyval| format!("Some({})", stringify_pyvalue_handle(pyval))
                                )
                            )
                        })
                        .join(",\n")
                        .as_str()
                )
            )
            .as_str(),
        );

        str.push_str(
            format!(
                "\n\tglobal_value_ids: {{{}}},",
                fmt_elems(
                    self.global_value_ids
                        .read()
                        .iter()
                        .sorted_by_key(|(k, _)| *k)
                        .map(|(k, v)| format!("\t\t{k} -> {v:?}"))
                        .join(",\n")
                        .as_str()
                )
            )
            .as_str(),
        );

        str.push_str(
            format!(
                "\n\tpyid_to_def: {{{}}},",
                fmt_elems(
                    self.pyid_to_def
                        .read()
                        .iter()
                        .sorted_by_key(|(k, _)| *k)
                        .map(|(k, v)| {
                            let tld_str = tld
                                .map_or_else(|| format!("{v:?}"), |tlds| stringify_tld(&tlds[v.0]));
                            format!("\t\t{k} -> {tld_str}")
                        })
                        .join(",\n")
                        .as_str()
                )
            )
            .as_str(),
        );

        str.push_str(
            format!(
                "\n\tpyid_to_type: {{{}}},",
                fmt_elems(
                    self.pyid_to_type
                        .read()
                        .iter()
                        .sorted_by_key(|(k, _)| *k)
                        .map(|(k, v)| {
                            let ty_str = unifier
                                .as_ref()
                                .map_or_else(|| format!("{v:?}"), |unifier| unifier.stringify(*v));
                            format!("\t\t{k} -> {ty_str}")
                        })
                        .join(",\n")
                        .as_str()
                )
            )
            .as_str(),
        );

        str.push_str(
            format!(
                "\n\tstring_store: {{{}}},",
                fmt_elems(
                    self.string_store
                        .read()
                        .iter()
                        .sorted_by_key(|(k, _)| *k)
                        .map(|(k, v)| format!("\t\t{k} -> {v}"))
                        .join(",\n")
                        .as_str()
                )
            )
            .as_str(),
        );

        str.push_str(
            format!(
                "\n\texception_ids: {{{}}},",
                fmt_elems(
                    self.exception_ids
                        .read()
                        .iter()
                        .sorted_by_key(|(k, _)| *k)
                        .map(|(k, v)| format!("\t\t{k} -> {v}"))
                        .join(",\n")
                        .as_str()
                )
            )
            .as_str(),
        );

        let name_to_pyid = &self.name_to_pyid;
        str.push_str(
            format!(
                "\n\tname_to_pyid: {{{}}},",
                fmt_elems(
                    name_to_pyid
                        .iter()
                        .sorted_by_cached_key(|(k, _)| k.to_string())
                        .map(|(k, v)| format!("\t\t{k} -> {v}"))
                        .join(",\n")
                        .as_str()
                )
            )
            .as_str(),
        );

        let module = &self.module;
        str.push_str(format!("\n\tmodule: {module}").as_str());

        str.push_str("\n}");

        str
    }
}
