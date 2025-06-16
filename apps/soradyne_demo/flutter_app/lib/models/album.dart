class Album {
  final String id;
  final String name;
  final int itemCount;
  final DateTime createdAt;

  Album({
    required this.id,
    required this.name,
    required this.itemCount,
    required this.createdAt,
  });

  factory Album.fromJson(Map<String, dynamic> json) {
    return Album(
      id: json['id'],
      name: json['name'],
      itemCount: json['item_count'],
      createdAt: DateTime.now(), // TODO: Parse from API
    );
  }
}
