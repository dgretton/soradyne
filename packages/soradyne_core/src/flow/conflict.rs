pub trait ConflictResolver<T> {
    fn resolve(&self, local: &T, remote: &T) -> T;
}

pub struct LastWriteWins;

impl<T: Clone> ConflictResolver<T> for LastWriteWins {
    fn resolve(&self, _local: &T, remote: &T) -> T {
        remote.clone()
    }
}

