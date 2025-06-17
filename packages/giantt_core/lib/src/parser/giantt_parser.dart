import 'dart:convert';
import '../models/giantt_item.dart';
import '../models/status.dart';
import '../models/priority.dart';
import '../models/duration.dart';
import '../models/relation.dart';
import '../models/time_constraint.dart';
import '../models/graph_exceptions.dart';

/// Main parser for Giantt file format
class GianttParser {
  /// Parse a line into a GianttItem
  static GianttItem fromString(String line, {bool occlude = false}) {
    line = line.strip();
    
    if (line.isEmpty || line.startsWith('#')) {
      throw GianttParseException('Empty or comment line', line);
    }

    try {
      // Parse the pre-title section
      final titleStart = line.indexOf('"');
      if (titleStart == -1) {
        throw GianttParseException('No opening quote found for title', line);
      }
      
      final preTitle = line.substring(0, titleStart).trim();
      final (status, idPriority, duration) = _parsePreTitleSection(preTitle);
      
      // Parse the title (JSON-escaped)
      final titleEnd = _findClosingQuote(line, titleStart);
      if (titleEnd == -1) {
        throw GianttParseException('No closing quote found for title', line);
      }
      
      final titleJson = line.substring(titleStart, titleEnd + 1);
      final title = jsonDecode(titleJson) as String;
      final postTitle = line.substring(titleEnd + 1).trim();
      
      // Extract ID and priority from id+priority string
      final (id, priority) = _parseIdPriority(idPriority);
      
      // Parse duration
      final parsedDuration = _parseDurationSafely(duration);
      
      // Parse post-title section
      final (charts, tags, relations, timeConstraint, userComment, autoComment) = 
          _parsePostTitleSection(postTitle);
      
      return GianttItem(
        id: id,
        title: title,
        status: status,
        priority: priority,
        duration: parsedDuration,
        charts: charts,
        tags: tags,
        relations: relations,
        timeConstraint: timeConstraint,
        userComment: userComment,
        autoComment: autoComment,
        occlude: occlude,
      );
    } catch (e) {
      if (e is GianttParseException) rethrow;
      throw GianttParseException('Parse error: $e', line);
    }
  }

  /// Convert a GianttItem to its string representation
  static String itemToString(GianttItem item) {
    final buffer = StringBuffer();
    
    // Status, ID+Priority, Duration
    buffer.write('${item.status.symbol} ${item.id}${item.priority.symbol} ${item.duration} ');
    
    // JSON-encoded title
    buffer.write(jsonEncode(item.title));
    
    // Charts
    buffer.write(' {');
    if (item.charts.isNotEmpty) {
      buffer.write('"${item.charts.join('","')}"');
    }
    buffer.write('}');
    
    // Tags
    if (item.tags.isNotEmpty) {
      buffer.write(' ${item.tags.join(',')}');
    }
    
    // Relations
    if (item.relations.isNotEmpty) {
      buffer.write(' >>> ');
      final relationParts = <String>[];
      
      for (final entry in item.relations.entries) {
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
    
    // Time constraint
    if (item.timeConstraint != null) {
      buffer.write(' @@@ ${item.timeConstraint}');
    }
    
    // Comments
    if (item.userComment != null && item.userComment!.isNotEmpty) {
      buffer.write(' # ${item.userComment}');
    }
    if (item.autoComment != null && item.autoComment!.isNotEmpty) {
      buffer.write(' ### ${item.autoComment}');
    }
    
    return buffer.toString();
  }

  /// Parse the pre-title section into status, id+priority, and duration
  static (GianttStatus, String, String) _parsePreTitleSection(String preTitle) {
    // Pattern: status id+priority duration
    final pattern = RegExp(r'^([○◑⊘●])\s+([^\s]+)\s+([^\s"]+)');
    final match = pattern.firstMatch(preTitle);
    
    if (match == null) {
      throw GianttParseException('Invalid pre-title format', preTitle);
    }
    
    final statusSymbol = match.group(1)!;
    final idPriority = match.group(2)!;
    final duration = match.group(3)!.trim();
    
    try {
      final status = GianttStatus.fromSymbol(statusSymbol);
      return (status, idPriority, duration);
    } catch (e) {
      throw GianttParseException('Invalid status symbol: $statusSymbol', preTitle);
    }
  }

  /// Parse ID and priority from combined string
  static (String, GianttPriority) _parseIdPriority(String idPriority) {
    // Priority symbols in order of length (longest first to avoid partial matches)
    const prioritySymbols = ['!!!', '!!', '!', '?', '...', ',,,'];
    
    for (final symbol in prioritySymbols) {
      if (idPriority.endsWith(symbol)) {
        final id = idPriority.substring(0, idPriority.length - symbol.length);
        final priority = GianttPriority.fromSymbol(symbol);
        return (id, priority);
      }
    }
    
    // No priority symbol found, default to neutral
    return (idPriority, GianttPriority.neutral);
  }

  /// Find the closing quote, handling escaped quotes
  static int _findClosingQuote(String line, int startPos) {
    var pos = startPos + 1;
    while (pos < line.length) {
      if (line[pos] == '"') {
        // Check if it's escaped
        var backslashCount = 0;
        var checkPos = pos - 1;
        while (checkPos >= 0 && line[checkPos] == '\\') {
          backslashCount++;
          checkPos--;
        }
        // If even number of backslashes (including 0), quote is not escaped
        if (backslashCount % 2 == 0) {
          return pos;
        }
      }
      pos++;
    }
    return -1;
  }

  /// Parse the post-title section
  static (List<String>, List<String>, Map<String, List<String>>, TimeConstraint?, String?, String?) 
      _parsePostTitleSection(String postTitle) {
    
    // Parse charts first
    final chartsPattern = RegExp(r'^\s*(\{[^}]*\})\s*(.*)$');
    final chartsMatch = chartsPattern.firstMatch(postTitle);
    
    if (chartsMatch == null) {
      throw GianttParseException('Invalid charts format', postTitle);
    }
    
    final chartsStr = chartsMatch.group(1)!;
    final remainder = chartsMatch.group(2)!;
    
    // Parse charts
    final charts = _parseCharts(chartsStr);
    
    // Split remainder by >>> to separate tags from relations
    final parts = remainder.split('>>>');
    final tagsStr = parts[0].trim();
    final relationsAndConstraints = parts.length > 1 ? parts[1].trim() : '';
    
    // Parse tags
    final tags = _parseTags(tagsStr);
    
    // Split relations section by @@@ to separate relations from time constraints
    final constraintParts = relationsAndConstraints.split('@@@');
    final relationsStr = constraintParts[0].trim();
    final timeConstraintStr = constraintParts.length > 1 ? constraintParts[1].trim() : null;
    
    // Parse relations
    final relations = _parseRelations(relationsStr);
    
    // Parse time constraint
    final timeConstraint = timeConstraintStr != null && timeConstraintStr.isNotEmpty
        ? TimeConstraint.fromString(timeConstraintStr)
        : null;
    
    // Parse comments (user # and auto ###)
    final (userComment, autoComment) = _parseComments(relationsAndConstraints);
    
    return (charts, tags, relations, timeConstraint, userComment, autoComment);
  }

  /// Parse charts from string like {"Chart1","Chart2"}
  static List<String> _parseCharts(String chartsStr) {
    if (chartsStr == '{}') return [];
    
    // Remove outer braces
    final inner = chartsStr.substring(1, chartsStr.length - 1);
    if (inner.trim().isEmpty) return [];
    
    // Split by comma and clean up
    return inner.split(',')
        .map((c) => c.trim().replaceAll('"', ''))
        .where((c) => c.isNotEmpty)
        .toList();
  }

  /// Parse tags from comma-separated string
  static List<String> _parseTags(String tagsStr) {
    if (tagsStr.isEmpty) return [];
    
    return tagsStr.split(',')
        .map((t) => t.trim())
        .where((t) => t.isNotEmpty)
        .toList();
  }

  /// Parse relations from string with symbols and brackets
  static Map<String, List<String>> _parseRelations(String relationsStr) {
    final relations = <String, List<String>>{};
    
    if (relationsStr.isEmpty) return relations;
    
    // Map symbols to relation type names
    final symbolToType = <String, String>{
      '⊢': 'REQUIRES',
      '⋲': 'ANYOF', 
      '≫': 'SUPERCHARGES',
      '∴': 'INDICATES',
      '∪': 'TOGETHER',
      '⊟': 'CONFLICTS',
      '►': 'BLOCKS',
      '≻': 'SUFFICIENT',
    };
    
    for (final entry in symbolToType.entries) {
      final symbol = entry.key;
      final relType = entry.value;
      
      final pattern = RegExp('${RegExp.escape(symbol)}\\[([^\\]]+)\\]');
      final matches = pattern.allMatches(relationsStr);
      
      for (final match in matches) {
        final targetsStr = match.group(1)!;
        final targets = targetsStr.split(',')
            .map((t) => t.trim())
            .where((t) => t.isNotEmpty)
            .toList();
        
        if (targets.isNotEmpty) {
          relations[relType] = targets;
        }
      }
    }
    
    return relations;
  }

  /// Parse comments from the relations and constraints section
  static (String?, String?) _parseComments(String text) {
    String? userComment;
    String? autoComment;
    
    // Look for auto comment first (triple ###) to avoid conflicts
    final autoCommentMatch = RegExp(r'###\s*(.*)$').firstMatch(text);
    if (autoCommentMatch != null) {
      autoComment = autoCommentMatch.group(1)?.trim();
      // Remove the auto comment from text to avoid conflicts with user comment parsing
      text = text.replaceFirst(RegExp(r'###.*$'), '').trim();
    }
    
    // Look for user comment (single # but not ###)
    final userCommentMatch = RegExp(r'#(?!##)\s*(.*)$').firstMatch(text);
    if (userCommentMatch != null) {
      userComment = userCommentMatch.group(1)?.trim();
    }
    
    return (userComment, autoComment);
  }

  /// Parse duration safely, wrapping errors in GianttParseException
  static GianttDuration _parseDurationSafely(String duration) {
    try {
      return GianttDuration.parse(duration);
    } catch (e) {
      throw GianttParseException('Invalid duration format: $duration', duration);
    }
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

/// Extension to add strip method to String
extension StringExtensions on String {
  String strip() => trim();
}
