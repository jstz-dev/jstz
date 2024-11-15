use utoipa::{
    openapi::{schema::ArrayItems, Discriminator, OneOf, RefOr, Schema},
    OpenApi,
};

#[derive(OpenApi)]
#[openapi(info(
    title = "Jstz Node",
    description = "JavaScript server runtime for Tezos Smart Rollups",
    license(
        name = "MIT",
        url = "https://github.com/jstz-dev/jstz/blob/main/LICENSE"
    ),
    contact(name = "Trilitech", email = "contact@trili.tech"),
))]
pub struct ApiDoc;

/// Modify OpenAPI doc after its been generated
pub fn modify(openapi: &mut utoipa::openapi::OpenApi) {
    if let Some(components) = &mut openapi.components {
        let schemas = &mut components.schemas;
        for (_, schema) in schemas.iter_mut() {
            modify_with_discrminator(schema);
        }
    }
}

/// Adds discriminator property to `oneOf` Schemas by checking if all of its variants
/// contain the `_type` discriminator property that was generate by serde + utoipa.
/// Recursively applies the modification to all nodes in the Schema tree
fn modify_with_discrminator(schema: &mut RefOr<Schema>) {
    match schema {
        RefOr::T(Schema::AllOf(all_of)) => {
            for all_of_schema in all_of.items.iter_mut() {
                modify_with_discrminator(all_of_schema)
            }
        }
        RefOr::T(Schema::AnyOf(any_of)) => {
            for item in any_of.items.iter_mut() {
                modify_with_discrminator(item)
            }
        }
        RefOr::T(Schema::OneOf(one_of)) => {
            if is_sum_type(one_of) {
                add_discriminator(one_of)
            }
        }
        RefOr::T(Schema::Array(array)) => match &mut array.items {
            ArrayItems::RefOrSchema(ref_or_schema) => {
                modify_with_discrminator(ref_or_schema)
            }
            ArrayItems::False => (),
        },
        RefOr::T(Schema::Object(obj)) => {
            for (_, property_schema) in obj.properties.iter_mut() {
                modify_with_discrminator(property_schema)
            }
        }
        RefOr::T(_) => (),
        RefOr::Ref(_) => (),
    }
}

/// Checks that all items in `one_of` are allOfs where at least
/// one member of the allOf set is an object with a single property
/// named "_type"
fn is_sum_type(one_of: &OneOf) -> bool {
    one_of.items.iter().all(|item| match item {
        RefOr::T(Schema::AllOf(all_of)) => {
            all_of.items.iter().any(|all_of_item| match all_of_item {
                RefOr::T(Schema::Object(obj)) => {
                    if obj.properties.len() != 1 {
                        return false;
                    }
                    obj.properties.contains_key("_type")
                }
                _ => false,
            })
        }
        _ => false,
    })
}

fn add_discriminator(one_of: &mut OneOf) {
    if one_of.discriminator.is_none() {
        let discrimator = Discriminator::new("_type");
        one_of.discriminator = Some(discrimator)
    }
}

#[cfg(test)]
mod test {

    use jstz_crypto::public_key_hash::PublicKeyHash;
    use jstz_proto::{operation::Content, receipt::ReceiptContent};
    use utoipa::{
        openapi::{
            schema::{ArrayItems, SchemaType},
            AllOfBuilder, ArrayBuilder, ComponentsBuilder, ObjectBuilder, OpenApi,
            OpenApiBuilder, RefOr, Schema, Type,
        },
        schema, PartialSchema,
    };

    use super::modify;

    fn unsafe_get_schema(
        open_api: &OpenApi,
        schema_name: impl Into<String>,
    ) -> RefOr<Schema> {
        open_api
            .components
            .clone()
            .unwrap()
            .schemas
            .get(schema_name.into().as_str())
            .unwrap()
            .clone()
    }

    fn check_contains_discriminator(schema: RefOr<Schema>) {
        assert!(matches!(schema, RefOr::T(Schema::OneOf(one_of))
             if one_of.discriminator.clone().unwrap().property_name == "_type"))
    }

    fn check_discriminator(
        open_api: &OpenApi,
        schema_name: impl Into<String>,
        discriminator_should_exist: bool,
    ) {
        let schema = unsafe_get_schema(open_api, schema_name);
        if discriminator_should_exist {
            check_contains_discriminator(schema)
        } else if let RefOr::T(Schema::OneOf(one_of)) = schema {
            assert!(one_of.discriminator.is_none())
        }
    }

    #[test]
    fn modify_discriminator_one_of() {
        let mut open_api = OpenApiBuilder::new()
            .components(Some(
                ComponentsBuilder::new()
                    .schema_from::<ReceiptContent>()
                    .schema_from::<Content>()
                    .build(),
            ))
            .build();

        modify(&mut open_api);
        check_discriminator(&open_api, "ReceiptContent", true);
        check_discriminator(&open_api, "Content", true);
    }

    #[test]
    fn modify_discriminator_one_of_non_discriminant_type() {
        let mut open_api = OpenApiBuilder::new()
            .components(Some(
                ComponentsBuilder::new()
                    .schema_from::<PublicKeyHash>()
                    .build(),
            ))
            .build();
        modify(&mut open_api);
        check_discriminator(&open_api, "PublicKeyHash", false);
    }

    #[test]
    fn modify_discriminator_on_all_of() {
        let mut open_api = OpenApiBuilder::new()
            .components(Some(
                ComponentsBuilder::new()
                    .schema(
                        "Test",
                        AllOfBuilder::new().item(
                            ObjectBuilder::new()
                                .schema_type(SchemaType::Type(Type::String)),
                        ),
                    )
                    .build(),
            ))
            .build();
        modify(&mut open_api);
        check_discriminator(&open_api, "Test", false);
    }

    #[test]
    fn modify_discriminator_on_array() {
        let mut open_api = OpenApiBuilder::new()
            .components(Some(
                ComponentsBuilder::new()
                    .schema("Test", ArrayBuilder::new().items(ReceiptContent::schema()))
                    .build(),
            ))
            .build();
        modify(&mut open_api);

        check_discriminator(&open_api, "Test", false);

        let array = unsafe_get_schema(&open_api, "Test");
        match array {
            RefOr::T(Schema::Array(arr)) => {
                if let ArrayItems::RefOrSchema(schema) = arr.items {
                    // Checks that inner inline schema definitions
                    // are modified
                    check_contains_discriminator(*schema)
                } else {
                    panic!("Expected schema")
                }
            }
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn modify_discrminator_on_object() {
        let mut open_api = OpenApiBuilder::new()
            .components(Some(
                ComponentsBuilder::new()
                    .schema(
                        "Test",
                        ObjectBuilder::new()
                            .property("test", schema!(Vec<String>))
                            .property("content", Content::schema())
                            .build(),
                    )
                    .build(),
            ))
            .build();
        modify(&mut open_api);
        check_discriminator(&open_api, "Test", false);

        let object = unsafe_get_schema(&open_api, "Test");
        match object {
            RefOr::T(Schema::Object(obj)) => {
                let content = obj.properties.get("content").unwrap();
                check_contains_discriminator(content.clone())
            }
            _ => panic!("Expected object"),
        }
    }
}
