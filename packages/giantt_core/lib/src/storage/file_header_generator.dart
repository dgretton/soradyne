import '../models/relation.dart';

/// Generates file headers and banners for Giantt files
class FileHeaderGenerator {
  /// Create a banner with text centered in a box of hash characters
  static String createBanner(String text, {int paddingH = 5, int paddingV = 1}) {
    final lines = text.split('\n');
    final maxLength = lines.map((line) => line.length).reduce((a, b) => a > b ? a : b);
    final bannerLen = maxLength + 2 * paddingH;
    final topBottomBorder = '#' * (bannerLen + 2);

    final buffer = StringBuffer();
    buffer.writeln(topBottomBorder);

    // Add vertical padding lines
    final emptyLine = '#${' ' * bannerLen}#';
    for (int i = 0; i < paddingV; i++) {
      buffer.writeln(emptyLine);
    }

    // Add text lines, centered
    for (final line in lines) {
      final padding = maxLength - line.length;
      final leftPadding = padding ~/ 2;
      final rightPadding = padding - leftPadding;
      buffer.writeln('#${' ' * paddingH}${' ' * leftPadding}$line${' ' * rightPadding}${' ' * paddingH}#');
    }

    // Add vertical padding lines
    for (int i = 0; i < paddingV; i++) {
      buffer.writeln(emptyLine);
    }

    buffer.writeln(topBottomBorder);
    return buffer.toString();
  }

  /// Generate header for items file
  static String generateItemsFileHeader() {
    return createBanner(
      'Giantt Items\n'
      'This file contains all include Giantt items in topological\n'
      'order according to the REQUIRES (${RelationType.requires.symbol}) relation.\n'
      'You can use #include directives at the top of this file\n'
      'to include other Giantt item files.\n'
      'Edit this file manually at your own risk.'
    );
  }

  /// Generate header for occluded items file
  static String generateOccludedItemsFileHeader() {
    return createBanner(
      'Giantt Occluded Items\n'
      'This file contains all occluded Giantt items in topological\n'
      'order according to the REQUIRES (${RelationType.requires.symbol}) relation.\n'
      'Edit this file manually at your own risk.'
    );
  }

  /// Generate header for logs file
  static String generateLogsFileHeader() {
    return createBanner(
      'Giantt Logs\n'
      'This file contains log entries in JSONL format.\n'
      'Each line is a JSON object representing a log entry.\n'
      'Edit this file manually at your own risk.'
    );
  }

  /// Generate header for occluded logs file
  static String generateOccludedLogsFileHeader() {
    return createBanner(
      'Giantt Occluded Logs\n'
      'This file contains occluded log entries in JSONL format.\n'
      'Each line is a JSON object representing a log entry.\n'
      'Edit this file manually at your own risk.'
    );
  }

  /// Generate header for metadata file
  static String generateMetadataFileHeader() {
    return createBanner(
      'Giantt Metadata\n'
      'This file contains metadata for the Giantt workspace.\n'
      'Edit this file manually at your own risk.'
    );
  }

  /// Generate a custom header with specified title and description
  static String generateCustomHeader(String title, String description) {
    return createBanner('$title\n$description');
  }

  /// Generate initialization timestamp comment
  static String generateTimestampComment() {
    final now = DateTime.now();
    return '# Generated on ${now.toIso8601String()}\n';
  }

  /// Generate include directive
  static String generateIncludeDirective(String includePath) {
    return '#include $includePath\n';
  }

  /// Generate file header with timestamp
  static String generateFileHeaderWithTimestamp(String headerType) {
    final header = switch (headerType.toLowerCase()) {
      'items' => generateItemsFileHeader(),
      'occluded_items' => generateOccludedItemsFileHeader(),
      'logs' => generateLogsFileHeader(),
      'occluded_logs' => generateOccludedLogsFileHeader(),
      'metadata' => generateMetadataFileHeader(),
      _ => generateCustomHeader('Giantt File', 'Generated file'),
    };
    
    return header + '\n' + generateTimestampComment() + '\n';
  }
}
