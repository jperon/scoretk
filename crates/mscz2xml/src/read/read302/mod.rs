//! Lecteur `.mscx` 3.x.
//!
//! Ce fichier est un bouchon : il sera remplacé par le lecteur 3.x écrit en
//! parallèle. Il ne contient volontairement que la signature publique et un
//! renvoi d'erreur.

/// Lit un `.mscx` 3.x.
///
/// Bouchon : renvoie toujours
/// [`Error::UnsupportedVersion`](crate::Error::UnsupportedVersion).
pub fn read(
    mscx: &[u8],
    version: crate::container::MscVersion,
) -> crate::Result<crate::model::Score> {
    let _ = (mscx, version);
    Err(crate::Error::UnsupportedVersion(version))
}
