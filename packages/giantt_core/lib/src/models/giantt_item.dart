import 'dart:convert';
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
    this.timeConstraints = const [],
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
  
  /// Time constraints for this item
  final List<TimeConstraint> timeConstraints;
  
  /// User-added comment
  final String? userComment;
  
  /// Auto-generated comment
  final String? autoComment;
  
  /// Whether this item is occluded (archived)
  final bool occlude;

  /// Set the occlude status of this item
  GianttItem setOcclude(bool occlude) {
    return copyWith(occlude: occlude);
  }

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
    List<TimeConstraint>? timeConstraints,
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
      timeConstraints: timeConstraints ?? this.timeConstraints,
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
           timeConstraints.length == other.timeConstraints.length &&
           timeConstraints.every((tc) => other.timeConstraints.contains(tc)) &&
           userComment == other.userComment &&
           autoComment == other.autoComment &&
           occlude == other.occlude;
  }

  @override
  int get hashCode => Object.hash(
    id, title, description, status, priority, duration,
    Object.hashAll(charts), Object.hashAll(tags),
    Object.hashAll(relations.entries), Object.hashAll(timeConstraints),
    userComment, autoComment, occlude
  );

  @override
  String toString() => 'GianttItem(id: $id, title: $title, status: $status)';

  /// Convert this item to its string representation for file storage
  String toFileString() {
    final buffer = StringBuffer();
    
    // Status, ID+Priority, Duration
    buffer.write('${status.symbol} $id${priority.symbol} $duration ');
    
    // JSON-encoded title
    buffer.write(jsonEncode(title));
    
    // Charts
    buffer.write(' {');
    if (charts.isNotEmpty) {
      buffer.write('"${charts.join('","')}"');
    } else {
      buffer.write('""');
    }
    buffer.write('}');
    
    // Tags
    if (tags.isNotEmpty) {
      buffer.write(' ${tags.join(',')}');
    }
    
    // Relations
    if (relations.isNotEmpty) {
      buffer.write(' >>> ');
      final relationParts = <String>[];
      
      for (final entry in relations.entries) {
        final relationType = entry.key;
        final targets = entry.value;
        if (targets.isNotEmpty) {
          // Find the symbol for this relation type
          final symbol = _getRelationSymbol(relationType);
          relationParts.add('$symbol[${targets.join(',')}]');
        }
      }
      buffer.write(relationParts.join(' '));
    }
    
    // Time constraints
    if (timeConstraints.isNotEmpty) {
      buffer.write(' @@@ ');
      buffer.write(timeConstraints.map((tc) => tc.toString()).join(' '));
    }
    
    // Comments
    if (userComment != null && userComment!.isNotEmpty) {
      buffer.write(' # $userComment');
    }
    if (autoComment != null && autoComment!.isNotEmpty) {
      buffer.write(' ### $autoComment');
    }
    
    return buffer.toString();
  }

  /// Get the symbol for a relation type name
  static String _getRelationSymbol(String relationType) {
    const typeToSymbol = {
      'REQUIRES': '⊢',
      'ANYOF': '⋲',
      'SUPERCHARGES': '≫',
      'INDICATES': '∴',
      'TOGETHER': '∪',
      'CONFLICTS': '⊟',
      'BLOCKS': '►',
      'SUFFICIENT': '≻',
    };
    
    return typeToSymbol[relationType] ?? '?';
  }
}
