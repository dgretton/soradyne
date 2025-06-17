import 'package:meta/meta.dart';
import 'status.dart';
import 'priority.dart';
import 'duration.dart';
import 'relation.dart';
import 'time_constraint.dart';

/// Represents a single Giantt item with all its properties
@immutable
class GianttItem {
  const GianttItem({
    required this.id,
    required this.title,
    this.description = '',
    this.status = GianttStatus.notStarted,
    this.priority = GianttPriority.neutral,
    required this.duration,
    this.charts = const [],
    this.tags = const [],
    this.relations = const {},
    this.timeConstraint,
    this.userComment,
    this.autoComment,
    this.occlude = false,
  });

  /// Unique identifier for this item
  final String id;
  
  /// Display title
  final String title;
  
  /// Optional description
  final String description;
  
  /// Current status
  final GianttStatus status;
  
  /// Priority level
  final GianttPriority priority;
  
  /// Estimated duration
  final GianttDuration duration;
  
  /// Charts this item belongs to
  final List<String> charts;
  
  /// Tags for categorization
  final List<String> tags;
  
  /// Relations to other items (relation type -> list of target IDs)
  final Map<String, List<String>> relations;
  
  /// Optional time constraint
  final TimeConstraint? timeConstraint;
  
  /// User-added comment
  final String? userComment;
  
  /// Auto-generated comment
  final String? autoComment;
  
  /// Whether this item is occluded (archived)
  final bool occlude;

  /// Create a copy with modified properties
  GianttItem copyWith({
    String? id,
    String? title,
    String? description,
    GianttStatus? status,
    GianttPriority? priority,
    GianttDuration? duration,
    List<String>? charts,
    List<String>? tags,
    Map<String, List<String>>? relations,
    TimeConstraint? timeConstraint,
    String? userComment,
    String? autoComment,
    bool? occlude,
  }) {
    return GianttItem(
      id: id ?? this.id,
      title: title ?? this.title,
      description: description ?? this.description,
      status: status ?? this.status,
      priority: priority ?? this.priority,
      duration: duration ?? this.duration,
      charts: charts ?? this.charts,
      tags: tags ?? this.tags,
      relations: relations ?? this.relations,
      timeConstraint: timeConstraint ?? this.timeConstraint,
      userComment: userComment ?? this.userComment,
      autoComment: autoComment ?? this.autoComment,
      occlude: occlude ?? this.occlude,
    );
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    if (other is! GianttItem) return false;
    return id == other.id &&
           title == other.title &&
           description == other.description &&
           status == other.status &&
           priority == other.priority &&
           duration == other.duration &&
           charts.length == other.charts.length &&
           charts.every((chart) => other.charts.contains(chart)) &&
           tags.length == other.tags.length &&
           tags.every((tag) => other.tags.contains(tag)) &&
           relations.length == other.relations.length &&
           timeConstraint == other.timeConstraint &&
           userComment == other.userComment &&
           autoComment == other.autoComment &&
           occlude == other.occlude;
  }

  @override
  int get hashCode => Object.hash(
    id, title, description, status, priority, duration,
    Object.hashAll(charts), Object.hashAll(tags),
    Object.hashAll(relations.entries), timeConstraint,
    userComment, autoComment, occlude
  );

  @override
  String toString() => 'GianttItem(id: $id, title: $title, status: $status)';
}
