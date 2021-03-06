use ast::*;
use name::*;
use expr::*;
use labeling::*;
use type_enforcement::*;

/// Parse a policy statement.
named!(pub statement<&[u8], Statement>,
    alt!(
        map!(declaration, Statement::Declaration) |
        map!(label, Statement::Label) |
        macro_call |
        if_else |
        allow_rule |
        set_modifier
    )
);

// Parse a list of 0 or more statements.
named!(pub statement_list<&[u8], Vec<Statement>>, many0!(statement));

// A statement that modifies a set or attribute with an expression.
named!(pub set_modifier<&[u8], Statement>,
    ws!(do_parse!(
        name: identifier >>
        tag!("|=") >>
        cast: delimited!(char!('('), type_specifier, char!(')')) >>
        expr: expr >>
        tag!(";") >> 

        (Statement::SetModifier{
            name,
            cast,
            expr: Box::from(expr)
        })
    ))
);

/// Parse either a block or symbol declaration.
named!(pub declaration<&[u8], Declaration>,
    alt!(
        block_declaration
        | macro_declaration
        | symbol_declaration 
        | class_declaration
    )
);

/// Parse a single named `Symbol` declaration.
named!(pub symbol_declaration<&[u8], Declaration>,
    ws!(do_parse!(
        qualifier: type_specifier >>
        name: identifier >>
        initializer: opt!(preceded!(tag!("="), expr)) >>
        char!(';') >>

        (Declaration::Symbol {qualifier, name, initializer})
    ))
);

// Parse a declaration of a `Class`, or `Common`, and it's collection of access vectors.
named!(pub class_declaration<&[u8], Declaration>,
    ws!(do_parse!(
        qualifier: type_specifier >>
        name: identifier >>
        extends: opt!(ws!(preceded!(tag!("extends"), identifier))) >> 
        access_vectors: delimited!(tag!("{"), many0!(identifier), tag!("}")) >>

        (Declaration::Class {qualifier, name, extends, access_vectors})
    ))
);

/// Parse a `block` or `optional` container, named by an `Identifer` and containing
/// a list of 0 or more `Statement`s.
named!(pub block_declaration<&[u8], Declaration>,
    ws!(do_parse!(
        is_abstract: opt!(tag!("abstract")) >>
        qualifier: type_specifier >>
        name: identifier >> 
        extends: opt!(complete!(
            ws!(do_parse!(
                 tag!("extends") >>
                 first: identifier >>
                 rest: many0!(ws!(do_parse!(char!(',') >> id: identifier >> (id)))) >>
        
                 ({
                     let mut extends_list = rest.clone();
                     extends_list.insert(0, first);
                     
                     extends_list
                 })
            ))
        )) >> 
        char!('{') >>
        statements: many0!(statement) >>
        char!('}') >>

        (Declaration::Block {
            is_abstract: is_abstract.is_some(),
            qualifier,
            name,
            statements,
            extends
        })
    ))
);

/// A declaration of a policy macro with a list of parameters.
named!(pub macro_declaration<&[u8], Declaration>,
    ws!(do_parse!(
        tag!("macro") >>
        name: identifier >>
        parameters: delimited!(tag!("("), macro_param_list, tag!(")")) >>
        statements: delimited!(tag!("{"), statement_list, tag!("}")) >>

        (Declaration::Macro {
            name,
            parameters,
            statements
        })
    ))
);

named!(pub macro_param_list<&[u8], Vec<MacroParameter>>,
    ws!(do_parse!(
        first_param: macro_param >>
        rest_params: many0!(ws!(do_parse!(char!(',') >> param: macro_param >> (param)))) >>
 
        ({
            let mut params = rest_params.clone();
            params.insert(0, first_param);
            
            params
        })
    ))
);

named!(pub macro_param<&[u8], MacroParameter>, 
    ws!(do_parse!(
        qualifier: type_specifier >>
        name: identifier >>

        (MacroParameter {
            qualifier, name
        })
    ))
);

named!(pub macro_call<&[u8], Statement>,
    ws!(do_parse!(
        name: identifier >>
        arguments: delimited!(tag!("("), macro_argument_list, tag!(")")) >>
        tag!(";") >>

        (Statement::MacroCall(name, arguments))
    ))
);

named!(pub macro_argument_list<&[u8], Vec<Expr>>,
    ws!(do_parse!(
        first_param: expr >>
        rest_params: many0!(ws!(do_parse!(char!(',') >> param: expr >> (param)))) >>
 
        ({
            let mut params = rest_params.clone();
            params.insert(0, first_param);
            
            params
        })
    ))
);

named!(pub if_else<Statement>,
    ws!(do_parse!(
        tag!("if") >>
        condition: expr >>
        then_block: delimited!(char!('{'), statement_list, char!('}')) >>
        else_ifs: many0!(else_if) >>
        else_block: opt!(complete!(
            ws!(do_parse!(
                tag!("else") >>
                block: delimited!(char!('{'), statement_list, char!('}')) >> 

                (block)
            ))
        )) >>

        (Statement::IfElse {
            condition,
            then_block,
            else_ifs,
            else_block,
        })
    ))
);

named!(pub else_if<(Expr, Vec<Statement>)>, 
    ws!(do_parse!(
        tag!("elseif") >>
        condition: expr >>
        then_block: delimited!(char!('{'), statement_list, char!('}')) >>

        (condition, then_block)
    ))
);

#[cfg(test)]
mod tests {

    use super::*;
    use testing::parse;

    #[test]
    pub fn parse_block_decl() {
        let result = parse::<Declaration, _>("block abc {}", block_declaration);
        let actual = Declaration::Block {
            is_abstract: false,
            qualifier: BlockType::Block,
            name: "abc".to_string(),
            statements: vec![],
            extends: None
        };

        assert_eq!(result, actual);
    }

    #[test]
    pub fn parse_abstract_block_decl() {
        let result = parse::<Declaration, _>("abstract block abc {}", block_declaration);
        let actual = Declaration::Block {
            is_abstract: true,
            qualifier: BlockType::Block,
            name: "abc".to_string(),
            statements: vec![],
            extends: None
        };

        assert_eq!(result, actual);
    }

    #[test]
    pub fn parse_block_decl_with_extends() {
        let result = parse::<Declaration, _>("block abc extends a, b {}", block_declaration);
        let expected = Declaration::Block {
            name: "abc".to_string(),
            qualifier: BlockType::Block,
            is_abstract: false,
            statements: vec![],
            extends: Some(vec!["a".to_string(), "b".to_string()]),
        };

        assert_eq!(expected, result);
    }

    #[test]
    pub fn parse_symbol_decl() {
        let result = parse::<Declaration, _>("type_attribute my_type;", symbol_declaration);
        let actual = Declaration::Symbol {
            qualifier: SymbolType::TypeAttribute,
            name: "my_type".to_string(),
            initializer: None
        };

        assert_eq!(result, actual);
    }

    #[test]
    pub fn parse_symbol_decl_with_initializer() {
        let result = parse::<Declaration, _>(
            "context my_context = user:role:type:s0-s1;",
            symbol_declaration,
        );

        let actual = Declaration::Symbol {
            qualifier: SymbolType::Context,
            name: "my_context".to_string(),
            initializer: Some(Expr::Context {
                user_id: "user".to_string(),
                role_id: "role".to_string(),
                type_id: "type".to_string(),
                level_range: Some(Box::from(Expr::LevelRange(Box::from(Expr::var("s0")), Box::from(Expr::var("s1")))))
            })
        };

        assert_eq!(result, actual);
    }

    #[test]
    pub fn parse_macro_decl() {
        let result = parse::<Declaration, _>(
            "macro my_macro(type v, type v1) {

            }",
            macro_declaration,
        );

        match result {
            Declaration::Macro { name, parameters, .. } => {
                assert_eq!("my_macro", name);
                assert_eq!("v", parameters[0].name);
                assert_eq!("v1", parameters[1].name);
            }
            _ => panic!("Invalid value parsed"),
        }
    }

    #[test]
    pub fn parse_macro_call() {
        let result = parse::<Statement, _>("my_macro(type_name);", macro_call);

        if let Statement::MacroCall(ref name, ref params) = result {
            assert_eq!("my_macro", name);
            assert_eq!(Expr::var("type_name"), params[0])
        } else {
            panic!("Invalid value parsed");
        }
    }

    #[test]
    pub fn parse_if_then_else() {
        let result = parse::<Statement, _>("if my_bool {} else{}", if_else);

        match result {
            Statement::IfElse {
                condition,
                else_block,
                ..
            } => {
                assert_eq!(Expr::var("my_bool"), condition);
                assert_eq!(Some(vec![]), else_block);
            }
            _ => panic!("Invalid value parsed"),
        }
    }

    #[test]
    pub fn parse_if() {
        let result = parse::<Statement, _>("if my_bool {}", if_else);

        match result {
            Statement::IfElse {
                condition,
                else_block,
                ..
            } => {
                assert_eq!(Expr::var("my_bool"), condition);
                assert_eq!(None, else_block);
            }
            _ => panic!("Invalid value parsed"),
        }
    }

    #[test]
    pub fn parse_set_modifier_stmt() {
        let result = parse::<Statement, _>("my_attrib |= (type) my_type;", set_modifier);
        let expected = Statement::SetModifier {
            name: "my_attrib".to_string(),
            cast: SymbolType::Type,
            expr: Box::from(Expr::var("my_type")),
        };

        assert_eq!(expected, result);
    }

    #[test]
    pub fn parse_class_decl() {
        let result = parse::<Declaration, _>("class file { read write }", class_declaration);
        let expected = Declaration::Class {
            qualifier: ClassType::Class,
            name: "file".to_string(),
            extends: None,
            access_vectors: vec![
                "read".to_string(),
                "write".to_string()
            ]
        };

        assert_eq!(expected, result);
    }

    #[test]
    pub fn parse_class_decl_with_extends() {
        let result = parse::<Declaration, _>("class file extends file_like {}", class_declaration);
        let expected = Declaration::Class {
            qualifier: ClassType::Class,
            name: "file".to_string(),
            extends: Some("file_like".to_string()),
            access_vectors: vec![]
        };

        assert_eq!(expected, result);
    }
}
