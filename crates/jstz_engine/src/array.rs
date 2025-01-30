use crate::{
    context::{CanAlloc, Context, InCompartment},
    custom_trace,
    gc::{
        ptr::{AsRawHandle, AsRawHandleMut, AsRawPtr, Handle, HandleMut},
        Compartment, Finalize, Prolong, Trace,
    },
    letroot,
    object::JsObject,
    value::JsValue,
};
use derive_more::Deref;
use mozjs::jsapi::{
    GetArrayLength, IsArrayObject, IsArrayObject1, JS_ValueToObject, NewArrayObject1,
};
struct JsArray<'a, C: Compartment> {
    inner: JsObject<'a, C>,
    length: u32,
}

impl<'a, C: Compartment> JsArray<'a, C> {
    pub fn new<S>(cx: &mut Context<S>) -> Self
    where
        S: InCompartment<C> + CanAlloc,
    {
        unsafe {
            let obj = NewArrayObject1(cx.as_raw_ptr(), 0);
            Self {
                inner: JsObject::from_raw(obj),
                length: 0,
            }
        }
    }

    #[allow(unused)]
    pub fn push<'cx, S>(
        &mut self,
        cx: &'cx mut Context<S>,
        value: &JsValue<'a, C>,
    ) -> bool
    where
        S: InCompartment<C> + CanAlloc,
    {
        letroot!(rooted_self = self.clone(); [cx]);
        letroot!(_rooted_val = value.clone(); [cx]);

        if let Some(true) = rooted_self
            .inner
            .set(ArrayIndex::new(self.length), value, cx)
        {
            self.length += 1;
            true
        } else {
            false
        }
    }

    pub fn from_obj<'b, S>(cx: &'a mut Context<S>, obj: &JsObject<'b, C>) -> Option<Self>
    where
        'a: 'b,
    {
        unsafe {
            letroot!(rooted_obj = obj.clone(); [cx]);
            let handle = rooted_obj.as_raw_handle();
            let is_array: *mut bool = std::ptr::null_mut();

            if IsArrayObject1(cx.as_raw_ptr(), handle, is_array) && *is_array {
                let length: *mut u32 = std::ptr::null_mut();
                if GetArrayLength(cx.as_raw_ptr(), handle, length) {
                    return Some(Self {
                        inner: rooted_obj.into_inner(cx),
                        length: length.read(),
                    });
                }
            }

            None
        }
    }

    pub fn from_value<'b, S>(
        cx: &'a mut Context<S>,
        value: &JsValue<'a, C>,
    ) -> Option<Self>
    where
        S: InCompartment<C> + CanAlloc,
        'a: 'b,
    {
        unsafe {
            letroot!(rooted_val = value.clone(); [cx]);
            let value_handle = rooted_val.as_raw_handle();
            let is_array: *mut bool = std::ptr::null_mut();

            if IsArrayObject(cx.as_raw_ptr(), value_handle, is_array) && *is_array {
                letroot!(obj = JsObject::new(cx); [cx]);
                let result = JS_ValueToObject(
                    cx.as_raw_ptr(),
                    value_handle,
                    obj.as_raw_handle_mut(),
                );
                if result {
                    let length: *mut u32 = std::ptr::null_mut();
                    if GetArrayLength(cx.as_raw_ptr(), obj.as_raw_handle(), length) {
                        return Some(Self {
                            inner: obj.into_inner(cx),
                            length: length.read(),
                        });
                    }
                }
            }
        }

        None
    }

    #[allow(unused)]
    pub fn into_vec<'cx, S>(self, cx: &'cx mut Context<S>) -> Vec<JsValue<'a, C>>
    where
        S: InCompartment<C> + CanAlloc,
    {
        letroot!(rooted_self = self.clone(); [cx]);
        let mut vec = Vec::with_capacity(self.length as usize);
        for i in 0..self.length {
            let index = ArrayIndex::new(i);
            let value = rooted_self.inner.get(index, cx).unwrap();
            vec.push(unsafe { value.extend_lifetime() });
        }
        vec
    }
}

impl<'a, C: Compartment> Clone for JsArray<'a, C> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            length: self.length,
        }
    }
}

impl<'a, C: Compartment> AsRawPtr for JsArray<'a, C> {
    type Ptr = <JsObject<'a, C> as AsRawPtr>::Ptr;

    unsafe fn as_raw_ptr(&self) -> Self::Ptr {
        self.inner.as_raw_ptr()
    }
}

impl<'a, C: Compartment> AsRawHandle for JsArray<'a, C> {
    unsafe fn as_raw_handle(&self) -> Handle<Self::Ptr> {
        self.inner.as_raw_handle()
    }
}

impl<'a, C: Compartment> AsRawHandleMut for JsArray<'a, C> {
    unsafe fn as_raw_handle_mut(&self) -> HandleMut<Self::Ptr> {
        self.inner.as_raw_handle_mut()
    }
}

unsafe impl<'a, 'b, C: Compartment> Prolong<'a> for JsArray<'b, C> {
    type Aged = JsArray<'a, C>;
}

impl<'a, C: Compartment> Finalize for JsArray<'a, C> {
    fn finalize(&self) {
        self.inner.finalize()
    }
}

unsafe impl<'a, C: Compartment> Trace for JsArray<'a, C> {
    custom_trace!(this, mark, {
        mark(&this.inner);
    });
}

#[derive(Deref)]
pub struct ArrayIndex(u32);

impl ArrayIndex {
    pub fn new(index: u32) -> Self {
        Self(index)
    }
}

#[cfg(test)]
mod test {

    use mozjs::jsval::StringValue;
    use mozjs::rust::{JSEngine, Runtime};

    use crate::array::JsArray;
    use crate::context::Context;
    use crate::gc::ptr::{AsRawHandle, AsRawPtr};
    use crate::letroot;
    use crate::string::str::JsStr;
    use crate::string::JsString;
    use crate::value::JsValue;

    #[test]
    fn test_eval_arr() {
        let engine = JSEngine::init().unwrap();
        let rt = Runtime::new(engine.handle());
        let mut base_cx = Context::from_runtime(&rt);
        let mut cx = base_cx.new_realm().unwrap();

        let s1 = JsString::from_slice(JsStr::latin1("hello world".as_bytes()), &cx);
        letroot!(rooted_s1 = s1; [cx]);
        let s2 = JsString::from_slice(JsStr::latin1("dlrow olleh".as_bytes()), &cx);
        letroot!(rooted_s2 = s2; [cx]);

        let s1_val = unsafe { JsValue::from_raw(StringValue(&*rooted_s1.as_raw_ptr())) };
        let s2_val = unsafe { JsValue::from_raw(StringValue(&*rooted_s2.as_raw_ptr())) };
        let mut js_array = JsArray::new(&mut cx);

        js_array.push(&mut cx, &s1_val);
        js_array.push(&mut cx, &s2_val);

        let vec = js_array.into_vec(&mut cx);
        println!("{:?}", vec.len());

        for js_val in vec.iter() {
            let js_string =
                unsafe { JsString::from_raw(js_val.as_raw_handle().to_string()) };
            let string = js_string.to_std_string(&cx).unwrap();
            println!("{}", string);
        }
    }
}
