import '../models/duration.dart';
import '../graph/giantt_graph.dart';

/// Additional validation issue types for comprehensive graph checking
enum ValidationSeverity {
  warning,
  error,
  info,
}

/// Validation issue categories
class ValidationIssue {
  const ValidationIssue({
    required this.type,
    required this.severity,
    required this.message,
    required this.itemId,
    this.suggestedFix,
    this.relatedItems = const [],
  });

  final String type;
  final ValidationSeverity severity;
  final String message;
  final String itemId;
  final String? suggestedFix;
  final List<String> relatedItems;
}

/// Validation rules for input checking
class ValidationRules {
  /// Check if an ID is valid (no special characters, not empty, etc.)
  static bool isValidId(String id) {
    if (id.isEmpty) return false;
    if (id.contains(' ')) return false;
    if (id.contains('\t')) return false;
    if (id.contains('\n')) return false;
    return true;
  }

  /// Check if a title is valid (not empty, reasonable length)
  static bool isValidTitle(String title) {
    if (title.isEmpty) return false;
    if (title.length > 200) return false; // Reasonable limit
    return true;
  }

  /// Check if a duration string is valid
  static bool isValidDuration(String duration) {
    try {
      GianttDuration.parse(duration);
      return true;
    } catch (e) {
      return false;
    }
  }

  /// Check for potential ID/title conflicts
  static List<ValidationIssue> checkIdTitleConflicts(GianttGraph graph, String newId, String newTitle) {
    final issues = <ValidationIssue>[];
    
    for (final item in graph.items.values) {
      // Check ID conflicts
      if (newId.toLowerCase() == item.title.toLowerCase()) {
        issues.add(ValidationIssue(
          type: 'id_title_conflict',
          severity: ValidationSeverity.error,
          message: 'ID "$newId" conflicts with existing item title',
          itemId: item.id,
          relatedItems: [item.id],
        ));
      }
      
      // Check title conflicts
      if (newTitle.toLowerCase() == item.title.toLowerCase()) {
        issues.add(ValidationIssue(
          type: 'title_conflict',
          severity: ValidationSeverity.error,
          message: 'Title "$newTitle" conflicts with existing item title',
          itemId: item.id,
          relatedItems: [item.id],
        ));
      }
    }
    
    return issues;
  }
}
