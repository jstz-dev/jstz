use crate::{
    js_class, js_constructor, js_entries, js_keys, js_method, js_values,
    sys::js::convert::Serde,
};
use deno_core::{v8, ByteString};

js_class!(Headers);

impl<'s> Headers<'s> {
    js_constructor! { fn new() }

    js_constructor! { fn new_with_headers(init: Headers<'s>) }

    js_constructor! { fn new_with_sequence(init: Serde<Vec<(ByteString, ByteString)>>) }

    js_method! { fn append(name: String, value: String) }

    js_method! { fn delete(name: String) }

    js_method! { fn get(name: String) -> Option<String> }

    js_method! {
        #[js_name(getSetCookie)]
        fn get_set_cookie(name: String) -> Serde<Vec<String>>
    }

    js_method! { fn has(name: String) -> bool }

    js_method! { fn set(name: String, value: String) }

    js_method! {
        #[js_name(forEach)]
        fn for_each(callback: v8::Local<'s, v8::Function>)
    }

    js_keys! {}

    js_values! {ByteString}

    js_entries!(ByteString);
}

#[cfg(test)]
mod test {
    use crate::init_test_setup;

    use super::*;

    #[test]
    fn test_new_and_append() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new(scope).unwrap();

        headers
            .append(scope, "Content-Type".into(), "application/json".into())
            .unwrap();

        assert_eq!(
            headers.get(scope, "Content-Type".into()).unwrap().unwrap(),
            "application/json"
        );
    }

    #[test]
    fn test_new_with_sequence() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new_with_sequence(
            scope,
            (vec![("Content-Type".into(), "application/json".into())]).into(),
        )
        .unwrap();

        assert_eq!(
            headers.get(scope, "Content-Type".into()).unwrap().unwrap(),
            "application/json"
        );
    }

    #[test]
    fn test_new_with_headers() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers1 = Headers::new(scope).unwrap();
        headers1
            .append(scope, "Content-Type".into(), "application/json".into())
            .unwrap();

        let headers2 = Headers::new_with_headers(scope, headers1.clone()).unwrap();

        assert_eq!(
            headers2
                .clone()
                .get(scope, "Content-Type".into())
                .unwrap()
                .unwrap(),
            "application/json"
        );

        assert_ne!(headers1, headers2);
    }

    #[test]
    fn test_append() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new(scope).unwrap();

        headers
            .append(scope, "Accept-Encoding".into(), "deflate".into())
            .unwrap();

        headers
            .append(scope, "Accept-Encoding".into(), "gzip".into())
            .unwrap();

        assert_eq!(
            headers
                .get(scope, "Accept-Encoding".into())
                .unwrap()
                .unwrap(),
            "deflate, gzip"
        );
    }

    #[test]
    fn test_delete() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new(scope).unwrap();

        headers
            .append(scope, "Content-Type".into(), "application/json".into())
            .unwrap();

        assert_eq!(
            headers.get(scope, "Content-Type".into()).unwrap().unwrap(),
            "application/json"
        );

        headers.delete(scope, "Content-Type".into()).unwrap();

        assert_eq!(headers.get(scope, "Content-Type".into()).unwrap(), None);
    }

    #[test]
    fn test_for_each() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new(scope).unwrap();

        headers
            .append(scope, "Content-Type".into(), "application/json".into())
            .unwrap();

        headers
            .append(scope, "Accept-Encoding".into(), "deflate".into())
            .unwrap();

        headers
            .append(scope, "Accept-Encoding".into(), "gzip".into())
            .unwrap();

        let mut results: Vec<(String, String)> = vec![];

        let callback = v8::FunctionBuilder::<v8::Function>::new(
            |scope: &mut v8::HandleScope,
             args: v8::FunctionCallbackArguments,
             _: v8::ReturnValue| {
                let name = args.get(0);
                let value = args.get(1);

                let name = name.to_string(scope).unwrap();
                let value = value.to_string(scope).unwrap();

                let name = name.to_rust_string_lossy(scope);
                let value = value.to_rust_string_lossy(scope);

                let result = (name, value);

                // push result to results
                let callback_results =
                    args.data().try_cast::<v8::External>().unwrap().value()
                        as *mut Vec<(String, String)>;

                let results = unsafe { &mut *(callback_results) };
                results.push(result);
            },
        )
        .data(v8::External::new(scope, &mut results as *const _ as *mut _).into())
        .build(scope)
        .unwrap();

        headers.for_each(scope, callback).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[1].1, "content-type");
        assert_eq!(results[1].0, "application/json");
        assert_eq!(results[0].1, "accept-encoding");
        assert_eq!(results[0].0, "deflate, gzip");
    }

    #[test]
    fn test_get_set_cookie() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new(scope).unwrap();

        headers
            .append(scope, "Set-Cookie".into(), "name1=value1".into())
            .unwrap();

        headers
            .append(scope, "Set-Cookie".into(), "name2=value2".into())
            .unwrap();

        let cookies = headers.get_set_cookie(scope, "Set-Cookie".into()).unwrap();

        assert_eq!(cookies.len(), 2);
        assert_eq!(cookies[0], "name1=value1");
        assert_eq!(cookies[1], "name2=value2");
    }

    #[test]
    fn test_has() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new(scope).unwrap();

        headers
            .append(scope, "Content-Type".into(), "application/json".into())
            .unwrap();

        assert!(headers.has(scope, "Content-Type".into()).unwrap());
        assert!(!headers.has(scope, "Accept-Encoding".into()).unwrap());
    }

    #[test]
    fn test_set() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new(scope).unwrap();

        headers
            .append(scope, "Content-Type".into(), "application/json".into())
            .unwrap();

        assert_eq!(
            headers.get(scope, "Content-Type".into()).unwrap().unwrap(),
            "application/json"
        );

        headers
            .set(scope, "Content-Type".into(), "text/html".into())
            .unwrap();

        assert_eq!(
            headers.get(scope, "Content-Type".into()).unwrap().unwrap(),
            "text/html"
        );
    }

    #[test]
    fn test_keys() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new(scope).unwrap();

        headers.append(scope, "key".into(), "value".into()).unwrap();

        let keys = headers.keys(scope).unwrap();

        let item = keys.next(scope).unwrap().unwrap();
        assert_eq!(item.as_ref(), b"key");

        assert_eq!(keys.next(scope).unwrap(), None);
    }

    #[test]
    fn test_values() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new(scope).unwrap();

        headers.append(scope, "key".into(), "value".into()).unwrap();

        let values = headers.values(scope).unwrap();

        let item = values.next(scope).unwrap().unwrap();
        assert_eq!(item.as_ref(), b"value");

        assert_eq!(values.next(scope).unwrap(), None);
    }

    #[test]
    fn test_entries() {
        init_test_setup! { runtime = runtime; };
        let scope = &mut runtime.handle_scope();

        let headers = Headers::new(scope).unwrap();

        headers.append(scope, "key".into(), "value".into()).unwrap();

        let entries = headers.entries(scope).unwrap();

        let item: (ByteString, ByteString) = entries.next(scope).unwrap().unwrap();
        assert_eq!(item.0.as_ref(), b"key");
        assert_eq!(item.1.as_ref(), b"value");

        assert_eq!(entries.next(scope).unwrap(), None);
    }
}
