pub mod model {
    include!(concat!(env!("OUT_DIR"), "/model.rs"));
}
pub mod persistence {
    include!(concat!(env!("OUT_DIR"), "/persistence.rs"));
}
pub mod storage {
    include!(concat!(env!("OUT_DIR"), "/storage.rs"));
}
