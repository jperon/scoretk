pub fn read(
    mscx: &[u8],
    version: crate::container::MscVersion,
) -> crate::Result<crate::model::Score> {
    let _ = (mscx, version);
    Err(crate::Error::UnsupportedVersion(version))
}
