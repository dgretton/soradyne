abstract class CrdtDocument<T> {
  String get id;
  T get state;
  Stream<T> get stateStream;
  
  Future<void> applyOperation(Map<String, dynamic> operation);
  Future<void> merge(CrdtDocument<T> other);
  Future<void> sync();
}

class CrdtAlbum extends CrdtDocument<AlbumState> {
  // Implementation that wraps your existing album CRDT
}

