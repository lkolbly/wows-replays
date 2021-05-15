//use crate::script_type::TypeAliases;
use crate::rpc::typedefs::{parse_aliases, parse_type, ArgType, ArgValue, TypeAliases};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flags {
    AllClients,
    CellPublicAndOwn,
    OwnClient,
    BaseAndClient,
    Base,
    CellPrivate,
    CellPublic,
    OtherClients,
}

impl Flags {
    fn from_str(s: &str) -> Self {
        if s == "ALL_CLIENTS" {
            Self::AllClients
        } else if s == "CELL_PUBLIC_AND_OWN" {
            Self::CellPublicAndOwn
        } else if s == "OWN_CLIENT" {
            Self::OwnClient
        } else if s == "BASE_AND_CLIENT" {
            Self::BaseAndClient
        } else if s == "BASE" {
            Self::Base
        } else if s == "CELL_PRIVATE" {
            Self::CellPrivate
        } else if s == "CELL_PUBLIC" {
            Self::CellPublic
        } else {
            panic!("Unrecognized flag {}!", s);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Property {
    pub name: String,
    pub prop_type: ArgType,
    flags: Flags,
}

#[derive(Debug)]
pub struct Method {
    pub name: String,
    variable_length_header_size: usize,
    pub args: Vec<ArgType>,
}

impl Method {
    fn sort_size(&self) -> usize {
        let size = self
            .args
            .iter()
            .map(|arg| arg.sort_size())
            .fold(0, |a, b| a + b);
        if size >= 0xffff {
            0xffff + self.variable_length_header_size
        } else {
            size + self.variable_length_header_size
        }
    }
}

struct DefFile {
    base_methods: Vec<Method>,
    cell_methods: Vec<Method>,
    client_methods: Vec<Method>,
    properties: Vec<Property>,
    implements: Vec<String>,
}

pub struct EntitySpec {
    name: String,
    base_methods: Vec<Method>,
    cell_methods: Vec<Method>,
    pub client_methods: Vec<Method>,
    pub properties: Vec<Property>,
}

fn child_by_name<'a, 'b>(
    node: &roxmltree::Node<'a, 'b>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'b>> {
    for child in node.children() {
        if child.tag_name().name() == name {
            return Some(child);
        }
    }
    None
}

fn parse_implements(ilist: &roxmltree::Node) -> Vec<String> {
    let mut implements = vec![];
    for implement in ilist.children() {
        if !implement.is_element() {
            continue;
        }

        implements.push(implement.text().unwrap().to_string());
    }
    implements
}

fn parse_properties(plist: &roxmltree::Node, aliases: &TypeAliases) -> Vec<Property> {
    let mut properties = vec![];
    for property in plist.children() {
        if !property.is_element() {
            continue;
        }

        properties.push(Property {
            name: property.tag_name().name().to_string(),
            prop_type: parse_type(&child_by_name(&property, "Type").unwrap(), aliases),
            flags: Flags::from_str(
                child_by_name(&property, "Flags")
                    .unwrap()
                    .text()
                    .unwrap()
                    .trim(),
            ),
        });
    }
    properties
}

fn parse_method(method: &roxmltree::Node, aliases: &TypeAliases) -> Method {
    let mut args = vec![];
    for child in method.children() {
        if child.tag_name().name() == "Arg" {
            //args.push(child.text().unwrap().to_string());
            args.push(parse_type(&child, aliases));
        }
        if child.tag_name().name() == "Args" {
            //println!("{:#?}", child);
            for child in child.children() {
                //println!("{:#?}", child);
                //if child.tag_name().name() == "Arg" {
                // The tag name is the arg name
                if child.is_element() {
                    args.push(parse_type(&child, aliases));
                }
                //}
            }
            //println!("{:#?}", args);
            //panic!();
        }
    }
    let variable_length_header_size = match child_by_name(&method, "VariableLengthHeaderSize") {
        Some(x) => {
            println!("{}: {:#?}", method.tag_name().name(), x.first_child());
            match x
                .first_child()
                .unwrap()
                .text()
                .unwrap()
                .trim()
                .parse::<usize>()
            {
                Ok(x) => x,
                Err(_) => 1,
            }
        }
        None => 1,
    };
    Method {
        name: method.tag_name().name().to_string(),
        variable_length_header_size,
        args: args,
    }
}

fn parse_method_list(mlist: &roxmltree::Node, aliases: &TypeAliases) -> Vec<Method> {
    let mut methods = vec![];
    for method in mlist.children() {
        if !method.is_element() {
            continue;
        }
        methods.push(parse_method(&method, aliases));
    }
    methods
}

fn parse_def(file: std::path::PathBuf, aliases: &TypeAliases) -> DefFile {
    let def = std::fs::read_to_string(&file).unwrap();
    let doc = roxmltree::Document::parse(&def).unwrap();
    let root = doc.root();
    let root = child_by_name(&root, "root").unwrap();
    //println!("{:?}", doc);

    // Parse out Implements, Properties, and ClientMethods
    let mut def = DefFile {
        base_methods: child_by_name(&root, "BaseMethods")
            .map(|n| parse_method_list(&n, aliases))
            .unwrap_or(vec![]),
        cell_methods: child_by_name(&root, "CellMethods")
            .map(|n| parse_method_list(&n, aliases))
            .unwrap_or(vec![]),
        client_methods: child_by_name(&root, "ClientMethods")
            .map(|n| parse_method_list(&n, aliases))
            .unwrap_or(vec![]),
        properties: child_by_name(&root, "Properties")
            .map(|n| parse_properties(&n, aliases))
            .unwrap_or(vec![]),
        implements: child_by_name(&root, "Implements")
            .map(|n| parse_implements(&n))
            .unwrap_or(vec![]),
    };
    def.client_methods.sort_by_key(|method| method.sort_size());
    def

    /*let mut implements = vec![];
    for child in root.first_child().unwrap().children() {
        //println!("{}", child.tag_name().name());
        if child.tag_name().name() == "Implements" {
            implements = parse_implements(&child);
            println!("Implements: {:#?}", implements);
        } else if child.tag_name().name() == "Properties" {
            let properties = parse_properties(&child);
            println!("{:#?}", properties);
        } else if child.tag_name().name() == "ClientMethods" {
            let mut client_methods = vec![];
            for method in child.children() {
                if !method.is_element() {
                    continue;
                }
                client_methods.push(parse_method(&method));
            }
            println!("{:#?}", client_methods);
        } else if child.tag_name().name() == "CellMethods" {
            let mut cell_methods = vec![];
            for method in child.children() {
                if !method.is_element() {
                    continue;
                }
                cell_methods.push(parse_method(&method));
            }
            println!("{:#?}", cell_methods);
        } else if child.tag_name().name() == "BaseMethods" {
            let mut base_methods = vec![];
            for method in child.children() {
                if !method.is_element() {
                    continue;
                }
                base_methods.push(parse_method(&method));
            }
            println!("{:#?}", base_methods);
        }
    }
    DefFile { implements }*/
}

pub fn parse_scripts(script_directory: std::path::PathBuf) -> Vec<EntitySpec> {
    let mut alias_path = script_directory.clone();
    alias_path.push("entity_defs");
    alias_path.push("alias.xml");

    let aliases = parse_aliases(&alias_path);

    let mut entities_xml_path = script_directory.clone();
    entities_xml_path.push("entities.xml");
    let entities_xml = std::fs::read_to_string(&entities_xml_path).unwrap();
    let doc = roxmltree::Document::parse(&entities_xml).unwrap();
    let root = doc.root();
    let mut entities = vec![];
    for child in child_by_name(
        &child_by_name(&root, "root").unwrap(),
        "ClientServerEntities",
    )
    .unwrap()
    .children()
    {
        if !child.is_element() {
            continue;
        }

        let mut def_path = script_directory.clone();
        def_path.push("entity_defs");
        def_path.push(&format!("{}.def", child.tag_name().name()));
        let mut def = parse_def(def_path, &aliases);
        let mut inherits = def
            .implements
            .iter()
            .map(|parent| {
                let mut parent_path = script_directory.clone();
                parent_path.push("entity_defs");
                parent_path.push("interfaces");
                parent_path.push(&format!("{}.def", parent));
                println!("Parsing parent {}...", parent);
                parse_def(parent_path, &aliases)
            })
            .flat_map(|mut parent| {
                // Sometimes, our parents have parents of our own. For now we only support
                // a single level of indirection.
                let mut result: Vec<_> = parent
                    .implements
                    .iter()
                    .map(|parent| {
                        let parent = parent.trim();
                        let mut parent_path = script_directory.clone();
                        parent_path.push("entity_defs");
                        parent_path.push("interfaces");
                        parent_path.push(&format!("{}.def", parent));
                        println!("Parsing parent {}...", parent);
                        parse_def(parent_path, &aliases)
                    })
                    .collect();
                parent.implements = vec![];
                result.push(parent);
                result
            })
            .fold(
                DefFile {
                    base_methods: vec![],
                    cell_methods: vec![],
                    client_methods: vec![],
                    properties: vec![],
                    implements: vec![],
                },
                |mut a, mut b| {
                    a.base_methods.append(&mut b.base_methods);
                    a.cell_methods.append(&mut b.cell_methods);
                    a.client_methods.append(&mut b.client_methods);
                    a.properties.append(&mut b.properties);
                    assert!(a.implements.len() == 0);
                    assert!(b.implements.len() == 0);
                    a
                },
            );
        println!(
            "{} has {} properties + {} inherited properties",
            child.tag_name().name(),
            def.properties.len(),
            inherits.properties.len()
        );

        let mut base_methods = inherits.base_methods;
        base_methods.append(&mut def.base_methods);

        let mut cell_methods = inherits.cell_methods;
        cell_methods.append(&mut def.cell_methods);

        let mut client_methods = inherits.client_methods;
        client_methods.append(&mut def.client_methods);

        client_methods.sort_by_key(|method| method.sort_size());

        let mut properties = inherits.properties;
        properties.append(&mut def.properties);

        properties = properties
            .iter()
            .filter(|property| {
                /*
                            EntityFlags.ALL_CLIENTS |
                            EntityFlags.BASE_AND_CLIENT |
                            EntityFlags.OTHER_CLIENTS |
                            EntityFlags.OWN_CLIENT |
                            EntityFlags.CELL_PUBLIC_AND_OWN |
                */
                property.flags == Flags::AllClients
                    || property.flags == Flags::BaseAndClient
                    || property.flags == Flags::OtherClients
                    || property.flags == Flags::OwnClient
                    || property.flags == Flags::CellPublicAndOwn
            })
            .map(|property| (*property).clone())
            .collect();

        properties.sort_by_key(|prop| prop.prop_type.sort_size());

        entities.push(EntitySpec {
            name: child.tag_name().name().to_string(),
            base_methods,
            cell_methods,
            client_methods,
            properties,
        });
    }

    for EntitySpec in entities.iter() {
        println!(
            "{} has {} properties and {}/{}/{} base/cell/client methods",
            EntitySpec.name,
            EntitySpec.properties.len(),
            EntitySpec.base_methods.len(),
            EntitySpec.cell_methods.len(),
            EntitySpec.client_methods.len()
        );

        if EntitySpec.name == "Vehicle" || EntitySpec.name == "Avatar" {
            for (i, property) in EntitySpec.properties.iter().enumerate() {
                println!(
                    " - {}: {} size={} type={:?}",
                    i,
                    property.name,
                    property.prop_type.sort_size(),
                    property.prop_type,
                );
            }
            println!("Methods:");
            for (i, method) in EntitySpec.client_methods.iter().enumerate() {
                println!(
                    " - {}: {}: size {} args: {:?}",
                    i,
                    method.name,
                    method.sort_size(),
                    method.args
                );
            }
        }
    }
    entities
}
