use boa_engine::Context;

use self::blob::BlobApi;
use self::file::FileApi as innerFileApi;

pub mod blob;
pub mod file;

pub struct FileApi;

impl jstz_core::Api for FileApi {
    fn init(self, context: &mut Context<'_>) {
        BlobApi.init(context);
        innerFileApi.init(context);
    }
}
