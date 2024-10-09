use boa_engine::Context;

use self::blob::BlobApi;
use self::imp::FileApi as ImpFileApi;

mod blob;
mod imp;

pub struct FileApi;

impl jstz_core::Api for FileApi {
    fn init(self, context: &mut Context) {
        BlobApi.init(context);
        ImpFileApi.init(context);
    }
}
