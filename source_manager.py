#!/usr/bin/env python3
"""
Soradyne Source Manager

This script helps manage the concatenated Soradyne source code file.
It allows you to:
- Generate a new concatenated file from all source files (Rust, HTML, Python, TOML)
- Add a new file to the existing concatenated file
- View all files currently included in the concatenation
"""

import os
import sys
import glob
import argparse
from datetime import datetime

class SoradyneManager:
    def __init__(self, source_dir='.', output_file='soradyne_complete.txt'):
        self.source_dir = source_dir
        self.output_file = output_file
        self.rust_files = []
        self.dart_files = []
        self.html_files = []
        self.python_files = []
        self.toml_files = []
        self.yaml_files = []
        self.shell_files = []
        self.markdown_files = []
        self.other_files = []
        self.excluded_dirs = ['.git', 'target', 'dist', 'node_modules', '__pycache__', 'build', '.dart_tool', 'ios', 'android', 'web', 'windows', 'linux', 'large_video_test', 'visual_block_test', 'renderer_test_output', 'heartrate_data', 'data']
        
        # Setup exclusion system
        self.exclusions_dir = os.path.join(self.source_dir, '.source_manager')
        self.exclusions_file = os.path.join(self.exclusions_dir, 'individual_exclusions.txt')
        self._ensure_exclusions_setup()
        self.individual_exclusions = self._load_exclusions()
        
    def collect_files(self):
        """Collect all Rust and TypeScript source files in the source directory"""
        # Find all the files currently included in the concatenated file, including those added manually
        included_files = self._currently_included_files()

        # Clear existing lists
        self.rust_files = []
        self.dart_files = []
        self.html_files = []
        self.python_files = []
        self.toml_files = []
        self.yaml_files = []
        self.shell_files = []
        self.markdown_files = []
        self.other_files = []
        
        # Start with key files
        self._add_if_exists('lib.rs', self.rust_files)
        self._add_if_exists('src/lib.rs', self.rust_files)
        self._add_if_exists('src/core/mod.rs', self.rust_files)
        self._add_if_exists('Cargo.toml', self.toml_files)
        self._add_if_exists('flutter_app/pubspec.yaml', self.yaml_files)
        self._add_if_exists('flutter_app/lib/main.dart', self.dart_files)
        self._add_if_exists('build_rust.sh', self.shell_files)
        
        # Add key Giantt core files
        self._add_if_exists('packages/giantt_core/bin/giantt.dart', self.dart_files)
        self._add_if_exists('packages/giantt_core/lib/giantt_core.dart', self.dart_files)
        self._add_if_exists('packages/giantt_core/pubspec.yaml', self.yaml_files)
        self._add_if_exists('docs/port_reference/giantt_cli.py', self.python_files)
        self._add_if_exists('docs/port_reference/giantt_core.py', self.python_files)
        
        # Find all source files in the source directory
        for root, dirs, files in os.walk(self.source_dir):
            # Skip excluded directories
            dirs[:] = [d for d in dirs if d not in self.excluded_dirs]
            
            for file in files:
                full_path = os.path.join(root, file)
                rel_path = os.path.relpath(full_path, self.source_dir)
                
                # Skip excluded files
                if rel_path in self.individual_exclusions:
                    continue
                
                # Collect Rust files
                if file.endswith('.rs'):
                    if rel_path not in self.rust_files:
                        self.rust_files.append(rel_path)
                
                # Collect Dart files
                elif file.endswith('.dart'):
                    if rel_path not in self.dart_files:
                        self.dart_files.append(rel_path)
                
                # Collect Python files
                elif file.endswith('.py'):
                    if rel_path not in self.python_files:
                        self.python_files.append(rel_path)
                
                # Collect TOML files
                elif file.endswith('.toml'):
                    if rel_path not in self.toml_files:
                        self.toml_files.append(rel_path)
                
                # Collect YAML files
                elif file.endswith('.yaml') or file.endswith('.yml'):
                    if rel_path not in self.yaml_files:
                        self.yaml_files.append(rel_path)
        
        for rel_path in included_files:
            # Skip excluded files
            if rel_path in self.individual_exclusions:
                continue
                
            if (rel_path not in self.rust_files and rel_path not in self.dart_files and 
                rel_path not in self.html_files and rel_path not in self.python_files and 
                rel_path not in self.toml_files and rel_path not in self.yaml_files and
                rel_path not in self.shell_files and rel_path not in self.markdown_files):
                if rel_path.endswith('.rs'):
                    self._add_if_exists(rel_path, self.rust_files)
                elif rel_path.endswith('.dart'):
                    self._add_if_exists(rel_path, self.dart_files)
                elif rel_path.endswith('.py'):
                    self._add_if_exists(rel_path, self.python_files)
                elif rel_path.endswith('.toml'):
                    self._add_if_exists(rel_path, self.toml_files)
                elif rel_path.endswith('.yaml') or rel_path.endswith('.yml'):
                    self._add_if_exists(rel_path, self.yaml_files)
                else:
                    self._add_if_exists(rel_path, self.other_files)
        
        total_files = len(self.rust_files) + len(self.dart_files) + len(self.html_files) + len(self.python_files) + len(self.toml_files) + len(self.yaml_files) + len(self.shell_files) + len(self.markdown_files) + len(self.other_files)
        print(f"Found {len(self.rust_files)} Rust files, {len(self.dart_files)} Dart files, {len(self.html_files)} HTML files, {len(self.python_files)} Python files, {len(self.toml_files)} TOML files, {len(self.yaml_files)} YAML files, {len(self.shell_files)} shell files, {len(self.markdown_files)} markdown files, and {len(self.other_files)} other files.")
        print(f"Excluded {len(self.individual_exclusions)} individual files.")
        return self.rust_files, self.html_files, self.python_files, self.toml_files, self.other_files
        
    def _ensure_exclusions_setup(self):
        """Ensure the .source_manager directory and exclusions file exist"""
        if not os.path.exists(self.exclusions_dir):
            os.makedirs(self.exclusions_dir)
        
        if not os.path.exists(self.exclusions_file):
            # Create default exclusions file
            with open(self.exclusions_file, 'w') as f:
                f.write("# Individual file exclusions for source_manager.py\n")
                f.write("# One file path per line, relative to project root\n")
                f.write("# Lines starting with # are comments and will be ignored\n\n")
    
    def _load_exclusions(self):
        """Load the list of excluded files"""
        if not os.path.exists(self.exclusions_file):
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
    
    def _add_if_exists(self, file_path, file_list):
        """Add a file to the list if it exists and is not excluded"""
        if file_path in self.individual_exclusions:
            return False
        
        full_path = os.path.join(self.source_dir, file_path)
        if os.path.exists(full_path):
            file_list.append(file_path)
            return True
        return False
    
    def _generate_directory_tree(self):
        """Generate a directory tree of included files"""
        all_files = (self.rust_files + self.dart_files + self.html_files + 
                    self.python_files + self.toml_files + self.yaml_files + 
                    self.shell_files + self.markdown_files + self.other_files)
        
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
        
        tree_lines = ["PROJECT DIRECTORY STRUCTURE (included files only):"]
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
            total_files = len(self.rust_files) + len(self.dart_files) + len(self.html_files) + len(self.python_files) + len(self.toml_files) + len(self.yaml_files) + len(self.shell_files) + len(self.markdown_files) + len(self.other_files)
            
            output.write("=====================================================================\n")
            output.write("= SORADYNE PROTOCOL - COMPLETE SOURCE CODE                         =\n")
            output.write(f"= Generated on: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}                           =\n")
            output.write(f"= Total files: {total_files}                                               =\n")
            output.write("=====================================================================\n\n")
            
            # Add directory tree
            output.write(self._generate_directory_tree())
            output.write("\n\n")
            
            # Add exclusions summary
            output.write("=====================================================================\n")
            output.write(self._generate_exclusions_summary())
            output.write("\n=====================================================================\n\n")
            
            # Add Rust files
            output.write("=====================================================================\n")
            output.write("= RUST SOURCE CODE                                                 =\n")
            output.write("=====================================================================\n\n")
            
            for file_path in self.rust_files:
                full_path = os.path.join(self.source_dir, file_path)
                
                output.write("\n\n// =====================================================================\n")
                output.write(f"// FILE: {file_path}\n")
                output.write("// =====================================================================\n\n")
                
                try:
                    with open(full_path, 'r', encoding='utf-8') as input_file:
                        output.write(input_file.read())
                except UnicodeDecodeError:
                    output.write(f"// [Error reading file: {file_path} - possible binary content]\n")
            
            # Add Dart files
            output.write("\n\n\n=====================================================================\n")
            output.write("= DART/FLUTTER SOURCE CODE                                        =\n")
            output.write("=====================================================================\n\n")
            
            for file_path in self.dart_files:
                full_path = os.path.join(self.source_dir, file_path)
                
                output.write("\n\n// =====================================================================\n")
                output.write(f"// FILE: {file_path}\n")
                output.write("// =====================================================================\n\n")
                
                try:
                    with open(full_path, 'r', encoding='utf-8') as input_file:
                        output.write(input_file.read())
                except UnicodeDecodeError:
                    output.write(f"// [Error reading file: {file_path} - possible binary content]\n")
            
            # Add HTML files
            output.write("\n\n\n=====================================================================\n")
            output.write("= HTML FILES                                                      =\n")
            output.write("=====================================================================\n\n")
            
            for file_path in self.html_files:
                full_path = os.path.join(self.source_dir, file_path)
                
                output.write("\n\n<!-- =====================================================================\n")
                output.write(f"FILE: {file_path}\n")
                output.write("===================================================================== -->\n\n")
                
                try:
                    with open(full_path, 'r', encoding='utf-8') as input_file:
                        output.write(input_file.read())
                except UnicodeDecodeError:
                    output.write(f"<!-- [Error reading file: {file_path} - possible binary content] -->\n")
            
            # Add Python files
            output.write("\n\n\n=====================================================================\n")
            output.write("= PYTHON FILES                                                    =\n")
            output.write("=====================================================================\n\n")
            
            for file_path in self.python_files:
                full_path = os.path.join(self.source_dir, file_path)
                
                output.write("\n\n# =====================================================================\n")
                output.write(f"# FILE: {file_path}\n")
                output.write("# =====================================================================\n\n")
                
                try:
                    with open(full_path, 'r', encoding='utf-8') as input_file:
                        output.write(input_file.read())
                except UnicodeDecodeError:
                    output.write(f"# [Error reading file: {file_path} - possible binary content]\n")
            
            # Add TOML files
            output.write("\n\n\n=====================================================================\n")
            output.write("= TOML FILES                                                      =\n")
            output.write("=====================================================================\n\n")
            
            for file_path in self.toml_files:
                full_path = os.path.join(self.source_dir, file_path)
                
                output.write("\n\n# =====================================================================\n")
                output.write(f"# FILE: {file_path}\n")
                output.write("# =====================================================================\n\n")
                
                try:
                    with open(full_path, 'r', encoding='utf-8') as input_file:
                        output.write(input_file.read())
                except UnicodeDecodeError:
                    output.write(f"# [Error reading file: {file_path} - possible binary content]\n")
            
            # Add YAML files
            output.write("\n\n\n=====================================================================\n")
            output.write("= YAML FILES                                                      =\n")
            output.write("=====================================================================\n\n")
            
            for file_path in self.yaml_files:
                full_path = os.path.join(self.source_dir, file_path)
                
                output.write("\n\n# =====================================================================\n")
                output.write(f"# FILE: {file_path}\n")
                output.write("# =====================================================================\n\n")
                
                try:
                    with open(full_path, 'r', encoding='utf-8') as input_file:
                        output.write(input_file.read())
                except UnicodeDecodeError:
                    output.write(f"# [Error reading file: {file_path} - possible binary content]\n")
            
            # Add shell files
            output.write("\n\n\n=====================================================================\n")
            output.write("= SHELL SCRIPTS                                                   =\n")
            output.write("=====================================================================\n\n")
            
            for file_path in self.shell_files:
                full_path = os.path.join(self.source_dir, file_path)
                
                output.write("\n\n# =====================================================================\n")
                output.write(f"# FILE: {file_path}\n")
                output.write("# =====================================================================\n\n")
                
                try:
                    with open(full_path, 'r', encoding='utf-8') as input_file:
                        output.write(input_file.read())
                except UnicodeDecodeError:
                    output.write(f"# [Error reading file: {file_path} - possible binary content]\n")
            
            # Add markdown files
            output.write("\n\n\n=====================================================================\n")
            output.write("= MARKDOWN FILES                                                  =\n")
            output.write("=====================================================================\n\n")
            
            for file_path in self.markdown_files:
                full_path = os.path.join(self.source_dir, file_path)
                
                output.write("\n\n<!-- =====================================================================\n")
                output.write(f"FILE: {file_path}\n")
                output.write("===================================================================== -->\n\n")
                
                try:
                    with open(full_path, 'r', encoding='utf-8') as input_file:
                        output.write(input_file.read())
                except UnicodeDecodeError:
                    output.write(f"<!-- [Error reading file: {file_path} - possible binary content] -->\n")
            
                # Add other files
                if self.other_files:
                    output.write("\n\n\n=====================================================================\n")
                    output.write("= OTHER FILES                                                     =\n")
                    output.write("=====================================================================\n\n")

                    for file_path in self.other_files:
                        full_path = os.path.join(self.source_dir, file_path)
                    
                        output.write("\n\n// =====================================================================\n")
                        output.write(f"// FILE: {file_path}\n")
                        output.write("// =====================================================================\n\n")
                    
                        try:
                            with open(full_path, 'r', encoding='utf-8') as input_file:
                                output.write(input_file.read())
                        except UnicodeDecodeError:
                            output.write(f"// [Error reading file: {file_path} - possible binary content]\n")

                output.write("\n\n// =====================================================================\n")
                output.write("= END OF FILES                                                    =\n")
                output.write("=====================================================================\n\n")

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
        if not os.path.exists(self.output_file):
            print(f"Error: Output file '{self.output_file}' does not exist. Generate it first.")
            return
            
        included_files = self._currently_included_files()
        if not included_files:
            print("No files currently included in the concatenation.")
            return
        
        print(f"Files included in {self.output_file}:")
        
        rust_files = [f for f in included_files if f.endswith('.rs')]
        dart_files = [f for f in included_files if f.endswith('.dart')]
        html_files = [f for f in included_files if f.endswith('.html')]
        python_files = [f for f in included_files if f.endswith('.py')]
        toml_files = [f for f in included_files if f.endswith('.toml')]
        yaml_files = [f for f in included_files if f.endswith('.yaml') or f.endswith('.yml')]
        shell_files = [f for f in included_files if f.endswith('.sh')]
        markdown_files = [f for f in included_files if f.endswith('.md')]
        other_files = [f for f in included_files if not (f.endswith('.rs') or f.endswith('.dart') or f.endswith('.html') or f.endswith('.py') or f.endswith('.toml') or f.endswith('.yaml') or f.endswith('.yml') or f.endswith('.sh') or f.endswith('.md'))]
        
        print("\nRust files:")
        for i, file_path in enumerate(rust_files):
            print(f"{i+1}. {file_path}")
        
        print("\nDart files:")
        for i, file_path in enumerate(dart_files):
            print(f"{i+1}. {file_path}")
        
        print("\nHTML files:")
        for i, file_path in enumerate(html_files):
            print(f"{i+1}. {file_path}")
        
        print("\nPython files:")
        for i, file_path in enumerate(python_files):
            print(f"{i+1}. {file_path}")
        
        print("\nTOML files:")
        for i, file_path in enumerate(toml_files):
            print(f"{i+1}. {file_path}")
        
        print("\nYAML files:")
        for i, file_path in enumerate(yaml_files):
            print(f"{i+1}. {file_path}")
        
        print("\nShell files:")
        for i, file_path in enumerate(shell_files):
            print(f"{i+1}. {file_path}")
        
        print("\nMarkdown files:")
        for i, file_path in enumerate(markdown_files):
            print(f"{i+1}. {file_path}")

        if other_files:
            print("\nOther files:")
            for i, file_path in enumerate(other_files):
                print(f"{i+1}. {file_path}")
        
        print(f"\nTotal: {len(included_files)} files ({len(rust_files)} Rust, {len(dart_files)} Dart, {len(html_files)} HTML, {len(python_files)} Python, {len(toml_files)} TOML, {len(yaml_files)} YAML, {len(shell_files)} Shell, {len(markdown_files)} Markdown)")

def main():
    parser = argparse.ArgumentParser(description='Manage Soradyne source code concatenation')
    parser.add_argument('--source', default='.', help='Source directory containing Rust and TypeScript files')
    parser.add_argument('--output', default='soradyne_complete.txt', help='Output file path')
    
    subparsers = parser.add_subparsers(dest='command', help='Command to execute')
    
    generate_parser = subparsers.add_parser('generate', help='Generate a new concatenated file')
    
    add_parser = subparsers.add_parser('add', help='Add a new file to the existing concatenated file')
    add_parser.add_argument('file', help='Path to the file to add')
    
    list_parser = subparsers.add_parser('list', help='List all files included in the concatenation')
    
    # Exclusion management commands
    exclude_parser = subparsers.add_parser('exclude', help='Add a file to the exclusions list')
    exclude_parser.add_argument('file', help='Path to the file to exclude')
    
    include_parser = subparsers.add_parser('include', help='Remove a file from the exclusions list')
    include_parser.add_argument('file', help='Path to the file to include')
    
    list_exclusions_parser = subparsers.add_parser('list-exclusions', help='List all excluded files')
    
    interactive_parser = subparsers.add_parser('interactive', help='Run in interactive mode')
    
    args = parser.parse_args()
    
    manager = SoradyneManager(args.source, args.output)
    
    if args.command == 'generate':
        manager.generate_concatenated_file()
    elif args.command == 'add':
        manager.add_file(args.file)
    elif args.command == 'list':
        manager.list_files()
    elif args.command == 'exclude':
        manager.add_exclusion(args.file)
    elif args.command == 'include':
        manager.remove_exclusion(args.file)
    elif args.command == 'list-exclusions':
        manager.list_exclusions()
    elif args.command == 'interactive':
        run_interactive_mode(manager)
    else:
        parser.print_help()

def run_interactive_mode(manager):
    """Run an interactive command loop"""
    print("Soradyne Source Manager - Interactive Mode")
    print("Type 'help' for a list of commands")
    
    while True:
        cmd = input("\nsoradyne> ").strip().lower()
        
        if cmd == 'exit' or cmd == 'quit':
            break
        elif cmd == 'help':
            print("Available commands:")
            print("  generate           - Generate a new concatenated file")
            print("  add <file>         - Add a new file to the concatenated file")
            print("  list               - List all files in the concatenation")
            print("  exclude <file>     - Add a file to the exclusions list")
            print("  include <file>     - Remove a file from the exclusions list")
            print("  list-exclusions    - List all excluded files")
            print("  exit/quit          - Exit the program")
        elif cmd == 'generate':
            manager.generate_concatenated_file()
        elif cmd.startswith('add '):
            file_path = cmd[4:].strip()
            if file_path:
                manager.add_file(file_path)
            else:
                print("Error: Please specify a file path")
        elif cmd == 'list':
            manager.list_files()
        elif cmd.startswith('exclude '):
            file_path = cmd[8:].strip()
            if file_path:
                manager.add_exclusion(file_path)
            else:
                print("Error: Please specify a file path")
        elif cmd.startswith('include '):
            file_path = cmd[8:].strip()
            if file_path:
                manager.remove_exclusion(file_path)
            else:
                print("Error: Please specify a file path")
        elif cmd == 'list-exclusions':
            manager.list_exclusions()
        elif cmd:
            print(f"Unknown command: '{cmd}'. Type 'help' for a list of commands.")

if __name__ == '__main__':
    main()
