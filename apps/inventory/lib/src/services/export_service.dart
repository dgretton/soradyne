import 'dart:io';
import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

class ExportService {
  /// Copies the provided text to the system clipboard
  static Future<void> copyToClipboard(String text) async {
    await Clipboard.setData(ClipboardData(text: text));
  }

  /// Saves the provided text to a file in the downloads directory
  static Future<String> saveToFile(String content, String filename) async {
    final directory = await getDownloadsDirectory() ?? await getApplicationDocumentsDirectory();
    final file = File('${directory.path}/$filename');
    await file.writeAsString(content);
    return file.path;
  }

  /// Saves content to a temp file and opens the OS share sheet.
  static Future<void> shareFile(String content, String filename) async {
    final tempDir = await getTemporaryDirectory();
    final file = File('${tempDir.path}/$filename');
    await file.writeAsString(content);
    await Share.shareXFiles([XFile(file.path)]);
  }
}
