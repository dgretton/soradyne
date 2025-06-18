#!/usr/bin/env python3
"""
Universal Source Manager

A configurable tool for collecting and concatenating source code files from any repository.
Designed to help prepare comprehensive source code documentation for LLM analysis.

Features:
- Configurable file type collection via JSON config
- Automatic discovery with customizable patterns
- Manual file addition and exclusion management
- Interactive and batch modes
- Flexible output formatting
- Project-agnostic design

Usage:
    source_manager.py init                    # Create default config
    source_manager.py generate               # Generate concatenated file
    source_manager.py add <file>             # Add specific file
    source_manager.py exclude <file>         # Exclude file from collection
    source_manager.py list                   # Show included files
    source_manager.py interactive            # Interactive mode
"""

import os
import sys
import glob
import argparse
import json
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Set, Optional, Any

class SourceManager:
    def __init__(self, source_dir='.', output_file=None, config_file=None):
        self.source_dir = Path(source_dir).resolve()
        self.config_file = config_file or self.source_dir / '.source_manager' / 'config.json'
        self.config = self._load_config()
        self.output_file = output_file or self.config.get('output_file', 'source_complete.txt')
        
        # File collections by category
        self.file_collections = {category: [] for category in self.config['file_types'].keys()}
        
        # Exclusion management
        self.exclusions_dir = self.source_dir / '.source_manager'
        self.exclusions_file = self.exclusions_dir / 'exclusions.txt'
        self._ensure_exclusions_setup()
        self.individual_exclusions = self._load_exclusions()
    
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
        """Return default configuration."""
        return {
            "project_name": "Source Code",
            "output_file": "source_complete.txt",
            "file_types": {
                "source": {
                    "extensions": [".py", ".js", ".ts", ".rs", ".dart", ".java", ".cpp", ".c", ".h", ".go"],
                    "description": "Source code files"
                },
                "config": {
                    "extensions": [".json", ".yaml", ".yml", ".toml", ".ini", ".cfg"],
                    "description": "Configuration files"
                },
                "markup": {
                    "extensions": [".html", ".xml", ".md", ".rst"],
                    "description": "Markup and documentation files"
                },
                "scripts": {
                    "extensions": [".sh", ".bat", ".ps1"],
                    "description": "Shell scripts and batch files"
                }
            },
            "excluded_dirs": [
                ".git", "node_modules", "__pycache__", ".pytest_cache",
                "target", "build", "dist", ".dart_tool", "coverage",
                "venv", "env", ".env", "vendor"
            ],
            "priority_files": [],
            "auto_include_patterns": [],
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
        
    def collect_files(self):
        """Collect files based on configuration."""
        # Find all the files currently included in the concatenated file
        included_files = self._currently_included_files()

        # Clear existing collections
        for category in self.file_collections:
            self.file_collections[category] = []
        
        # Add priority files first
        for priority_file in self.config.get('priority_files', []):
            self._add_priority_file(priority_file)
        
        # Auto-discover files by walking the directory tree
        for root, dirs, files in os.walk(self.source_dir):
            # Skip excluded directories
            dirs[:] = [d for d in dirs if d not in self.config['excluded_dirs']]
            
            for file in files:
                full_path = Path(root) / file
                rel_path = full_path.relative_to(self.source_dir)
                
                # Skip excluded files
                if str(rel_path) in self.individual_exclusions:
                    continue
                
                # Categorize file by extension
                self._categorize_and_add_file(str(rel_path), file)
        
        # Add any manually included files that weren't found
        for rel_path in included_files:
            if str(rel_path) in self.individual_exclusions:
                continue
            
            # Check if file is already in a collection
            already_included = any(str(rel_path) in collection for collection in self.file_collections.values())
            if not already_included:
                file_name = Path(rel_path).name
                self._categorize_and_add_file(str(rel_path), file_name)
        
        # Print summary
        total_files = sum(len(collection) for collection in self.file_collections.values())
        category_counts = {cat: len(files) for cat, files in self.file_collections.items() if files}
        
        print(f"Found {total_files} files:")
        for category, count in category_counts.items():
            description = self.config['file_types'][category]['description']
            print(f"  {count} {description.lower()}")
        print(f"Excluded {len(self.individual_exclusions)} individual files.")
        
        return self.file_collections
    
    def _add_priority_file(self, file_path: str):
        """Add a priority file to the appropriate collection."""
        if self._add_if_exists(file_path):
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
        
    def _ensure_exclusions_setup(self):
        """Ensure the .source_manager directory and exclusions file exist"""
        self.exclusions_dir.mkdir(parents=True, exist_ok=True)
        
        if not self.exclusions_file.exists():
            # Create default exclusions file
            with open(self.exclusions_file, 'w') as f:
                f.write("# Individual file exclusions for source_manager\n")
                f.write("# One file path per line, relative to project root\n")
                f.write("# Lines starting with # are comments and will be ignored\n\n")
    
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
    
    def _save_exclusions(self):
        """Save the current exclusions list to file"""
        with open(self.exclusions_file, 'w') as f:
            f.write("# Individual file exclusions for source_manager.py\n")
            f.write("# One file path per line, relative to project root\n")
            f.write("# Lines starting with # are comments and will be ignored\n\n")
            for exclusion in sorted(self.individual_exclusions):
                f.write(f"{exclusion}\n")
    
    def add_exclusion(self, file_path):
        """Add a file to the exclusions list"""
        if file_path not in self.individual_exclusions:
            self.individual_exclusions.append(file_path)
            self._save_exclusions()
            print(f"Added '{file_path}' to exclusions list")
            return True
        else:
            print(f"'{file_path}' is already in exclusions list")
            return False
    
    def remove_exclusion(self, file_path):
        """Remove a file from the exclusions list"""
        if file_path in self.individual_exclusions:
            self.individual_exclusions.remove(file_path)
            self._save_exclusions()
            print(f"Removed '{file_path}' from exclusions list")
            return True
        else:
            print(f"'{file_path}' is not in exclusions list")
            return False
    
    def list_exclusions(self):
        """List all currently excluded files"""
        if not self.individual_exclusions:
            print("No files are currently excluded.")
            return
        
        print("Currently excluded files:")
        for i, exclusion in enumerate(sorted(self.individual_exclusions), 1):
            print(f"{i}. {exclusion}")
        print(f"\nTotal: {len(self.individual_exclusions)} excluded files")
    
    def _add_if_exists(self, file_path):
        """Check if a file exists and is not excluded"""
        if file_path in self.individual_exclusions:
            return False
        
        full_path = self.source_dir / file_path
        return full_path.exists()
    
    def _generate_directory_tree(self):
        """Generate a directory tree of included files"""
        all_files = []
        for collection in self.file_collections.values():
            all_files.extend(collection)
        
        # Build directory structure
        tree = {}
        for file_path in sorted(all_files):
            parts = file_path.split('/')
            current = tree
            for part in parts[:-1]:  # All but the last part (filename)
                if part not in current:
                    current[part] = {}
                current = current[part]
            # Add the file
            current[parts[-1]] = None
        
        # Convert to string representation
        def format_tree(node, prefix="", is_last=True):
            lines = []
            items = list(node.items())
            for i, (name, subtree) in enumerate(items):
                is_last_item = i == len(items) - 1
                current_prefix = "└── " if is_last_item else "├── "
                lines.append(f"{prefix}{current_prefix}{name}")
                
                if subtree is not None:  # It's a directory
                    extension = "    " if is_last_item else "│   "
                    lines.extend(format_tree(subtree, prefix + extension, is_last_item))
            return lines
        
        tree_lines = [f"{self.config['project_name'].upper()} DIRECTORY STRUCTURE (included files only):"]
        tree_lines.append(".")
        tree_lines.extend(format_tree(tree))
        return "\n".join(tree_lines)
    
    def _generate_exclusions_summary(self):
        """Generate a summary of excluded files"""
        if not self.individual_exclusions:
            return "No files are currently excluded."
        
        lines = [f"EXCLUDED FILES ({len(self.individual_exclusions)} total):"]
        lines.append("")
        
        # Group by category based on path patterns
        categories = {
            "Examples": [],
            "Flutter/Dart": [],
            "Binaries": [],
            "Bindings": [],
            "Storage": [],
            "Network": [],
            "Video": [],
            "Album": [],
            "Flow": [],
            "Other": []
        }
        
        for exclusion in sorted(self.individual_exclusions):
            if exclusion.startswith("examples/"):
                categories["Examples"].append(exclusion)
            elif exclusion.startswith("flutter_app/"):
                categories["Flutter/Dart"].append(exclusion)
            elif exclusion.startswith("src/bin/"):
                categories["Binaries"].append(exclusion)
            elif exclusion.startswith("src/bindings/"):
                categories["Bindings"].append(exclusion)
            elif exclusion.startswith("src/storage/"):
                categories["Storage"].append(exclusion)
            elif exclusion.startswith("src/network/"):
                categories["Network"].append(exclusion)
            elif exclusion.startswith("src/video/"):
                categories["Video"].append(exclusion)
            elif exclusion.startswith("src/album/"):
                categories["Album"].append(exclusion)
            elif exclusion.startswith("src/flow/"):
                categories["Flow"].append(exclusion)
            else:
                categories["Other"].append(exclusion)
        
        for category, files in categories.items():
            if files:
                lines.append(f"{category}:")
                for file in files:
                    lines.append(f"  - {file}")
                lines.append("")
        
        return "\n".join(lines)
        
    def generate_concatenated_file(self):
        """Generate a new concatenated file from all source files"""
        self.collect_files()
        
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
            
            # Add directory tree
            output.write(self._generate_directory_tree())
            output.write("\n\n")
            
            # Add exclusions summary
            output.write("=====================================================================\n")
            output.write(self._generate_exclusions_summary())
            output.write("\n=====================================================================\n\n")
            
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
                
                for file_path in files:
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
            
            # Footer
            border = "=" * 70
            output.write(f"\n\n{border}\n")
            output.write(f"= {'END OF FILES':<66} =\n")
            output.write(f"{border}\n\n")

        print(f"Concatenation complete! Output file: {self.output_file}")
        
    def add_file(self, file_path):
        """Add a new file to the existing concatenated file"""
        full_path = os.path.join(self.source_dir, file_path)
        
        if not os.path.exists(full_path):
            print(f"Error: File '{file_path}' does not exist")
            return False
            
        if not os.path.exists(self.output_file):
            print(f"Error: Output file '{self.output_file}' does not exist. Generate it first.")
            return False
        
        with open(self.output_file, 'a') as output:
            output.write("\n\n// =====================================================================\n")
            output.write(f"// FILE: {file_path}\n")
            output.write("// =====================================================================\n\n")
            
            try:
                with open(full_path, 'r', encoding='utf-8') as input_file:
                    output.write(input_file.read())
            except UnicodeDecodeError:
                output.write(f"// [Error reading file: {file_path} - possible binary content]\n")
        
        print(f"Added file '{file_path}' to {self.output_file}")
        return True
        
    def _currently_included_files(self):
        """Get a list of currently included files in the concatenated file"""
        if not os.path.exists(self.output_file):
            print(f"(Output file '{self.output_file}' does not exist.)")
            return []
        
        included_files = []
        with open(self.output_file, 'r', encoding='utf-8') as f:
            for line in f:
                if line.startswith('// FILE: '):
                    included_files.append(line[9:].strip())
        
        return included_files
    
    def list_files(self):
        """List all files currently included in the concatenation"""
        if not Path(self.output_file).exists():
            print(f"Error: Output file '{self.output_file}' does not exist. Generate it first.")
            return
            
        included_files = self._currently_included_files()
        if not included_files:
            print("No files currently included in the concatenation.")
            return
        
        print(f"Files included in {self.output_file}:")
        
        # Group files by category
        categorized_files = {category: [] for category in self.config['file_types'].keys()}
        uncategorized_files = []
        
        for file_path in included_files:
            ext = Path(file_path).suffix.lower()
            categorized = False
            
            for category, config in self.config['file_types'].items():
                if ext in config['extensions']:
                    categorized_files[category].append(file_path)
                    categorized = True
                    break
            
            if not categorized:
                uncategorized_files.append(file_path)
        
        # Print categorized files
        total_count = 0
        for category, files in categorized_files.items():
            if files:
                description = self.config['file_types'][category]['description']
                print(f"\n{description}:")
                for i, file_path in enumerate(files, 1):
                    print(f"{i:3}. {file_path}")
                total_count += len(files)
        
        # Print uncategorized files
        if uncategorized_files:
            print(f"\nOther files:")
            for i, file_path in enumerate(uncategorized_files, 1):
                print(f"{i:3}. {file_path}")
            total_count += len(uncategorized_files)
        
        # Print summary
        category_summary = []
        for category, files in categorized_files.items():
            if files:
                category_summary.append(f"{len(files)} {category}")
        
        if uncategorized_files:
            category_summary.append(f"{len(uncategorized_files)} other")
        
        print(f"\nTotal: {total_count} files ({', '.join(category_summary)})")
    
    def init_config(self):
        """Initialize configuration file with defaults."""
        if self.config_file.exists():
            print(f"Configuration already exists at {self.config_file}")
            return False
        
        self.config = self._default_config()
        self._save_config()
        print(f"Created default configuration at {self.config_file}")
        print("Edit this file to customize file types, exclusions, and other settings.")
        return True
    
    def show_config(self):
        """Display current configuration."""
        print("Current configuration:")
        print(json.dumps(self.config, indent=2))
    
    def add_file_type(self, category: str, extensions: List[str], description: str):
        """Add a new file type category."""
        self.config['file_types'][category] = {
            'extensions': extensions,
            'description': description
        }
        self._save_config()
        print(f"Added file type category '{category}' with extensions: {', '.join(extensions)}")

def main():
    parser = argparse.ArgumentParser(
        description='Universal Source Manager - Collect and concatenate source code files',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s init                     # Create default configuration
  %(prog)s generate                 # Generate concatenated source file
  %(prog)s add src/main.py          # Add specific file
  %(prog)s exclude tests/           # Exclude directory
  %(prog)s list                     # Show included files
  %(prog)s config                   # Show current configuration
  %(prog)s interactive              # Interactive mode

Configuration:
  The tool uses .source_manager/config.json for settings.
  Run 'init' to create a default configuration file.
        """
    )
    parser.add_argument('--source', default='.', help='Source directory (default: current directory)')
    parser.add_argument('--output', help='Output file path (overrides config)')
    parser.add_argument('--config', help='Configuration file path')
    
    subparsers = parser.add_subparsers(dest='command', help='Available commands')
    
    # Init command
    init_parser = subparsers.add_parser('init', help='Initialize configuration file')
    
    # Generate command
    generate_parser = subparsers.add_parser('generate', help='Generate concatenated source file')
    
    # Add command
    add_parser = subparsers.add_parser('add', help='Add specific file to collection')
    add_parser.add_argument('file', help='Path to the file to add')
    
    # List command
    list_parser = subparsers.add_parser('list', help='List all included files')
    
    # Config commands
    config_parser = subparsers.add_parser('config', help='Show current configuration')
    
    # Exclusion management
    exclude_parser = subparsers.add_parser('exclude', help='Add file/pattern to exclusions')
    exclude_parser.add_argument('file', help='Path to the file to exclude')
    
    include_parser = subparsers.add_parser('include', help='Remove file from exclusions')
    include_parser.add_argument('file', help='Path to the file to include')
    
    list_exclusions_parser = subparsers.add_parser('list-exclusions', help='List excluded files')
    
    # File type management
    add_type_parser = subparsers.add_parser('add-type', help='Add new file type category')
    add_type_parser.add_argument('category', help='Category name')
    add_type_parser.add_argument('extensions', nargs='+', help='File extensions (e.g., .py .pyx)')
    add_type_parser.add_argument('--description', required=True, help='Category description')
    
    # Interactive mode
    interactive_parser = subparsers.add_parser('interactive', help='Run in interactive mode')
    
    args = parser.parse_args()
    
    if not args.command:
        parser.print_help()
        return
    
    manager = SourceManager(args.source, args.output, args.config)
    
    if args.command == 'init':
        manager.init_config()
    elif args.command == 'generate':
        manager.generate_concatenated_file()
    elif args.command == 'add':
        manager.add_file(args.file)
    elif args.command == 'list':
        manager.list_files()
    elif args.command == 'config':
        manager.show_config()
    elif args.command == 'exclude':
        manager.add_exclusion(args.file)
    elif args.command == 'include':
        manager.remove_exclusion(args.file)
    elif args.command == 'list-exclusions':
        manager.list_exclusions()
    elif args.command == 'add-type':
        manager.add_file_type(args.category, args.extensions, args.description)
    elif args.command == 'interactive':
        run_interactive_mode(manager)

def run_interactive_mode(manager):
    """Run an interactive command loop"""
    print("Universal Source Manager - Interactive Mode")
    print("Type 'help' for a list of commands")
    
    while True:
        cmd = input("\nsource> ").strip()
        
        if cmd.lower() in ['exit', 'quit']:
            break
        elif cmd.lower() == 'help':
            print("Available commands:")
            print("  generate              - Generate a new concatenated file")
            print("  add <file>            - Add a new file to the collection")
            print("  list                  - List all files in the concatenation")
            print("  config                - Show current configuration")
            print("  exclude <file>        - Add a file to the exclusions list")
            print("  include <file>        - Remove a file from the exclusions list")
            print("  list-exclusions       - List all excluded files")
            print("  add-type <cat> <exts> - Add new file type category")
            print("  exit/quit             - Exit the program")
        elif cmd.lower() == 'generate':
            manager.generate_concatenated_file()
        elif cmd.lower().startswith('add '):
            file_path = cmd[4:].strip()
            if file_path:
                manager.add_file(file_path)
            else:
                print("Error: Please specify a file path")
        elif cmd.lower() == 'list':
            manager.list_files()
        elif cmd.lower() == 'config':
            manager.show_config()
        elif cmd.lower().startswith('exclude '):
            file_path = cmd[8:].strip()
            if file_path:
                manager.add_exclusion(file_path)
            else:
                print("Error: Please specify a file path")
        elif cmd.lower().startswith('include '):
            file_path = cmd[8:].strip()
            if file_path:
                manager.remove_exclusion(file_path)
            else:
                print("Error: Please specify a file path")
        elif cmd.lower() == 'list-exclusions':
            manager.list_exclusions()
        elif cmd.lower().startswith('add-type '):
            parts = cmd[9:].strip().split()
            if len(parts) >= 2:
                category = parts[0]
                extensions = parts[1:]
                description = input(f"Description for '{category}': ").strip()
                if description:
                    manager.add_file_type(category, extensions, description)
                else:
                    print("Error: Description is required")
            else:
                print("Error: Usage: add-type <category> <extension1> [extension2] ...")
        elif cmd.strip():
            print(f"Unknown command: '{cmd}'. Type 'help' for a list of commands.")

if __name__ == '__main__':
    main()
