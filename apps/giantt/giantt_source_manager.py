#!/usr/bin/env python3
"""
Giantt Source Manager

A specialized version of source_manager.py configured for the Giantt project.
This script collects all Giantt-related source code from across the monorepo
into a single concatenated file for LLM analysis.

Usage:
    ./giantt_source_manager.py init                    # Create default config
    ./giantt_source_manager.py generate               # Generate concatenated file
    ./giantt_source_manager.py add <file>             # Add specific file
    ./giantt_source_manager.py exclude <file>         # Exclude file from collection
    ./giantt_source_manager.py list                   # Show included files
"""

import os
import sys
import glob
import argparse
import json
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Set, Optional, Any

class GianttSourceManager:
    def __init__(self, source_dir='.', output_file=None, config_file=None):
        self.source_dir = Path(source_dir).resolve()
        self.config_file = config_file or self.source_dir / '.source_manager' / 'config.json'
        self.config = self._load_config()
        self.output_file = output_file or self.config.get('output_file', 'giantt_complete.txt')
        
        # File collections by category
        self.file_collections = {category: [] for category in self.config['file_types'].keys()}
        
        # Exclusion and priority file management
        self.exclusions_dir = self.source_dir / '.source_manager'
        self.exclusions_file = self.exclusions_dir / 'exclusions.txt'
        self.priority_file = self.exclusions_dir / 'priority.txt'
        self._ensure_exclusions_setup()
        self._ensure_priority_setup()
        self.individual_exclusions = self._load_exclusions()
        self.priority_files = self._load_priority_files()
    
    def _load_config(self) -> Dict[str, Any]:
        """Load configuration from JSON file or create default."""
        if self.config_file.exists():
            try:
                with open(self.config_file, 'r') as f:
                    return json.load(f)
            except (json.JSONDecodeError, FileNotFoundError):
                print(f"Warning: Could not load config from {self.config_file}, using defaults")
        
        return self._default_config()
    
    def _default_config(self) -> Dict[str, Any]:
        """Return default configuration for Giantt project."""
        return {
            "project_name": "Giantt Project",
            "output_file": "giantt_complete.txt",
            "file_types": {
                "dart_source": {
                    "extensions": [".dart"],
                    "description": "Dart source code files"
                },
                "flutter_config": {
                    "extensions": [".yaml", ".yml"],
                    "description": "Flutter and Dart configuration files"
                },
                "android_config": {
                    "extensions": [".kt", ".properties", ".gradle"],
                    "description": "Android configuration files"
                },
                "python_reference": {
                    "extensions": [".py"],
                    "description": "Python reference implementation"
                },
                "documentation": {
                    "extensions": [".md", ".rst", ".txt"],
                    "description": "Documentation files"
                }
            },
            "excluded_dirs": [
                ".git", ".dart_tool", "build", ".gradle", "android/app/build",
                "android/.gradle", "ios", ".idea", ".vscode", "coverage", ".packages"
            ],
            "auto_include_patterns": [
                "../../packages/giantt_core/**/*.dart",
                "../../docs/port_reference/giantt_core.py"
            ],
            "banner_config": {
                "padding_h": 5,
                "padding_v": 1,
                "char": "="
            }
        }
    
    def _save_config(self):
        """Save current configuration to file."""
        self.exclusions_dir.mkdir(parents=True, exist_ok=True)
        with open(self.config_file, 'w') as f:
            json.dump(self.config, f, indent=2)
    
    def _ensure_exclusions_setup(self):
        """Ensure the .source_manager directory and exclusions file exist"""
        self.exclusions_dir.mkdir(parents=True, exist_ok=True)
        
        if not self.exclusions_file.exists():
            with open(self.exclusions_file, 'w') as f:
                f.write("# Individual file exclusions for giantt_source_manager\n")
                f.write("# One file path per line, relative to project root\n")
                f.write("# Lines starting with # are comments and will be ignored\n\n")
                f.write("# Common output files\n")
                f.write("giantt_complete.txt\n")
                f.write("*.log\n")
                f.write("*.tmp\n")
                f.write("\n# Generated files\n")
                f.write("android/app/src/main/java/io/flutter/plugins/GeneratedPluginRegistrant.java\n")
                f.write("android/local.properties\n")
                f.write(".flutter-plugins\n")
                f.write(".flutter-plugins-dependencies\n")
                f.write(".packages\n")
                f.write("pubspec.lock\n")
    
    def _ensure_priority_setup(self):
        """Ensure the priority files file exists"""
        if not self.priority_file.exists():
            with open(self.priority_file, 'w') as f:
                f.write("# Priority files for giantt_source_manager\n")
                f.write("# These files will always be included in the output\n")
                f.write("# One file path per line, relative to project root\n")
                f.write("# Lines starting with # are comments and will be ignored\n\n")
                f.write("# Core Giantt package files\n")
                f.write("../../packages/giantt_core/lib/giantt_core.dart\n")
                f.write("../../packages/giantt_core/lib/src/models/giantt_item.dart\n")
                f.write("../../packages/giantt_core/lib/src/models/duration.dart\n")
                f.write("../../packages/giantt_core/lib/src/models/priority.dart\n")
                f.write("../../packages/giantt_core/lib/src/models/status.dart\n")
                f.write("\n# Python reference implementation\n")
                f.write("../../docs/port_reference/giantt_core.py\n")
                f.write("\n# Main app files\n")
                f.write("lib/main.dart\n")
                f.write("pubspec.yaml\n")
    
    def _load_exclusions(self):
        """Load the list of excluded files"""
        if not self.exclusions_file.exists():
            return []
        
        with open(self.exclusions_file, 'r') as f:
            exclusions = []
            for line in f:
                line = line.strip()
                if line and not line.startswith('#'):
                    exclusions.append(line)
            return exclusions
    
    def _load_priority_files(self):
        """Load the list of priority files"""
        if not self.priority_file.exists():
            return []
        
        with open(self.priority_file, 'r') as f:
            priority_files = []
            for line in f:
                line = line.strip()
                if line and not line.startswith('#'):
                    priority_files.append(line)
            return priority_files
    
    def _is_excluded(self, file_path: str) -> bool:
        """Check if a file path matches any exclusion pattern"""
        import fnmatch
        
        # Always exclude the configured output file
        output_filename = Path(self.output_file).name
        if Path(file_path).name == output_filename:
            return True
        
        for exclusion in self.individual_exclusions:
            if file_path == exclusion or fnmatch.fnmatch(file_path, exclusion):
                return True
            if Path(file_path).name == exclusion:
                return True
        
        return False
    
    def collect_files(self, debug=False):
        """Collect files from Giantt app, core package, and reference docs."""
        # Clear existing collections
        for category in self.file_collections:
            self.file_collections[category] = []
        
        # Add priority files first
        for priority_file in self.priority_files:
            self._add_priority_file(priority_file)
        
        # Collect from current directory (apps/giantt)
        self._collect_from_directory(self.source_dir, debug=debug)
        
        # Collect from giantt_core package
        giantt_core_path = self.source_dir / "../../packages/giantt_core"
        if giantt_core_path.exists():
            self._collect_from_directory(giantt_core_path.resolve(), debug=debug, prefix="../../packages/giantt_core/")
        
        # Collect Python reference
        python_ref_path = self.source_dir / "../../docs/port_reference/giantt_core.py"
        if python_ref_path.exists():
            rel_path = "../../docs/port_reference/giantt_core.py"
            if not self._is_excluded(rel_path):
                self._categorize_and_add_file(rel_path, "giantt_core.py")
        
        # Print summary
        total_files = sum(len(collection) for collection in self.file_collections.values())
        category_counts = {cat: len(files) for cat, files in self.file_collections.items() if files}
        
        print(f"Found {total_files} Giantt-related files:")
        for category, count in category_counts.items():
            description = self.config['file_types'][category]['description']
            print(f"  {count} {description.lower()}")
        print(f"Excluded {len(self.individual_exclusions)} individual files.")
        
        return self.file_collections
    
    def _collect_from_directory(self, directory: Path, debug=False, prefix=""):
        """Collect files from a specific directory."""
        for root, dirs, files in os.walk(directory):
            # Skip excluded directories
            dirs[:] = [d for d in dirs if d not in self.config['excluded_dirs']]
            
            for file in files:
                full_path = Path(root) / file
                if prefix:
                    rel_path = prefix + str(full_path.relative_to(directory))
                else:
                    rel_path = str(full_path.relative_to(self.source_dir))
                
                # Skip excluded files
                if self._is_excluded(rel_path):
                    if debug:
                        print(f"DEBUG: Excluded file: {rel_path}")
                    continue
                
                # Skip hidden files and common build artifacts
                if file.startswith('.') or file.endswith('.lock'):
                    continue
                
                self._categorize_and_add_file(rel_path, file)
    
    def _add_priority_file(self, file_path: str):
        """Add a priority file to the appropriate collection."""
        full_path = self.source_dir / file_path
        if full_path.exists() and not self._is_excluded(file_path):
            file_name = Path(file_path).name
            self._categorize_and_add_file(file_path, file_name, force=True)
    
    def _categorize_and_add_file(self, rel_path: str, file_name: str, force: bool = False):
        """Categorize a file by extension and add to appropriate collection."""
        file_ext = Path(file_name).suffix.lower()
        
        for category, config in self.config['file_types'].items():
            if file_ext in config['extensions']:
                if rel_path not in self.file_collections[category]:
                    self.file_collections[category].append(rel_path)
                return
        
        # If no category matches and force is True, add to 'other' category
        if force:
            if 'other' not in self.file_collections:
                self.file_collections['other'] = []
            if rel_path not in self.file_collections['other']:
                self.file_collections['other'].append(rel_path)
    
    def generate_concatenated_file(self, debug=False):
        """Generate a new concatenated file from all Giantt-related source files"""
        self.collect_files(debug=debug)
        
        with open(self.output_file, 'w') as output:
            total_files = sum(len(collection) for collection in self.file_collections.values())
            
            # Generate header
            project_name = self.config['project_name'].upper()
            header_text = f"{project_name} - COMPLETE SOURCE CODE"
            border_char = self.config['banner_config']['char']
            border_len = max(70, len(header_text) + 10)
            border = border_char * border_len
            
            output.write(f"{border}\n")
            output.write(f"{border_char} {header_text:<{border_len-4}} {border_char}\n")
            output.write(f"{border_char} Generated on: {datetime.now().strftime('%Y-%m-%d %H:%M:%S'):<{border_len-16}} {border_char}\n")
            output.write(f"{border_char} Total files: {total_files:<{border_len-15}} {border_char}\n")
            output.write(f"{border}\n\n")
            
            # Write files by category
            for category, files in self.file_collections.items():
                if not files:
                    continue
                
                category_config = self.config['file_types'][category]
                section_title = category_config['description'].upper()
                
                border = "=" * 70
                output.write(f"\n\n{border}\n")
                output.write(f"= {section_title:<66} =\n")
                output.write(f"{border}\n\n")
                
                for file_path in sorted(files):
                    # Determine the actual file path to read
                    if file_path.startswith('../../'):
                        full_path = self.source_dir / file_path
                    else:
                        full_path = self.source_dir / file_path
                    
                    # Choose comment style based on file extension
                    ext = Path(file_path).suffix.lower()
                    if ext in ['.html', '.xml', '.md']:
                        comment_start = "<!-- "
                        comment_end = " -->"
                    elif ext in ['.py', '.sh', '.yaml', '.yml', '.toml']:
                        comment_start = "# "
                        comment_end = ""
                    else:
                        comment_start = "// "
                        comment_end = ""
                    
                    output.write(f"\n\n{comment_start}{'=' * 69}{comment_end}\n")
                    output.write(f"{comment_start}FILE: {file_path}{comment_end}\n")
                    output.write(f"{comment_start}{'=' * 69}{comment_end}\n\n")
                    
                    try:
                        with open(full_path, 'r', encoding='utf-8') as input_file:
                            output.write(input_file.read())
                    except UnicodeDecodeError:
                        output.write(f"{comment_start}[Error reading file: {file_path} - possible binary content]{comment_end}\n")
                    except FileNotFoundError:
                        output.write(f"{comment_start}[Error: File not found: {file_path}]{comment_end}\n")
            
            # Footer
            border = "=" * 70
            output.write(f"\n\n{border}\n")
            output.write(f"= {'END OF FILES':<66} =\n")
            output.write(f"{border}\n\n")

        print(f"Giantt source concatenation complete! Output file: {self.output_file}")
    
    def init_config(self):
        """Initialize configuration file with Giantt-specific defaults."""
        if self.config_file.exists():
            print(f"Configuration already exists at {self.config_file}")
            return False
        
        self.config = self._default_config()
        self._save_config()
        print(f"Created Giantt-specific configuration at {self.config_file}")
        print("Edit this file to customize file types, exclusions, and other settings.")
        return True

def main():
    parser = argparse.ArgumentParser(
        description='Giantt Source Manager - Collect Giantt-related source code files',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s init                     # Create Giantt-specific configuration
  %(prog)s generate                 # Generate concatenated source file
  %(prog)s generate --debug         # Generate with debug output

This tool is specifically configured to collect:
- Flutter app code from apps/giantt/
- Core Dart package from packages/giantt_core/
- Python reference from docs/port_reference/giantt_core.py
        """
    )
    parser.add_argument('--debug', action='store_true', help='Enable debug output')
    
    subparsers = parser.add_subparsers(dest='command', help='Available commands')
    
    # Init command
    init_parser = subparsers.add_parser('init', help='Initialize Giantt-specific configuration')
    
    # Generate command
    generate_parser = subparsers.add_parser('generate', help='Generate concatenated source file')
    generate_parser.add_argument('--debug', action='store_true', help='Enable debug output')
    
    args = parser.parse_args()
    
    if not args.command:
        parser.print_help()
        return
    
    manager = GianttSourceManager()
    
    if args.command == 'init':
        manager.init_config()
    elif args.command == 'generate':
        debug = getattr(args, 'debug', False)
        manager.generate_concatenated_file(debug=debug)

if __name__ == '__main__':
    main()
