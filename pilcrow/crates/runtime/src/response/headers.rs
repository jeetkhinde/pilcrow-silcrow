use headers::{Error, Header, HeaderName, HeaderValue};
use std::iter;

/// Macro to easily define a custom string-valued header for Pilcrow.
macro_rules! define_string_header {
    ($struct_name:ident, $header_name:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $struct_name(pub String);

        impl $struct_name {
            #[allow(dead_code)]
            pub(crate) const NAME: &'static str = $header_name;
        }

        impl Header for $struct_name {
            fn name() -> &'static HeaderName {
                static NAME: HeaderName = HeaderName::from_static($header_name);
                &NAME
            }

            fn decode<'i, I>(values: &mut I) -> Result<Self, Error>
            where
                I: Iterator<Item = &'i HeaderValue>,
            {
                let value = values.next().ok_or_else(Error::invalid)?;
                let s = value.to_str().map_err(|_| Error::invalid())?;
                Ok($struct_name(s.to_string()))
            }

            fn encode<E: Extend<HeaderValue>>(&self, values: &mut E) {
                if let Ok(value) = HeaderValue::from_str(&self.0) {
                    values.extend(iter::once(value));
                }
            }
        }
    };
}

// Define the standard set of Silcrow headers as strongly-typed wrappers

define_string_header!(SilcrowTarget, "silcrow-target");
define_string_header!(SilcrowCache, "silcrow-cache");
define_string_header!(SilcrowTrigger, "silcrow-trigger");
define_string_header!(SilcrowRetarget, "silcrow-retarget");
define_string_header!(SilcrowPush, "silcrow-push");
define_string_header!(SilcrowPatch, "silcrow-patch");
define_string_header!(SilcrowInvalidate, "silcrow-invalidate");
define_string_header!(SilcrowNavigate, "silcrow-navigate");
define_string_header!(SilcrowSse, "silcrow-sse");
define_string_header!(SilcrowWs, "silcrow-ws");
