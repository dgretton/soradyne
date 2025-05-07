#!/usr/bin/env python3
"""
Soradyne Source Manager

This script helps manage the concatenated Soradyne source code file.
It allows you to:
- Generate a new concatenated file from all source files (Rust and TypeScript)
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
        self.ts_files = []
        self.excluded_dirs = ['.git', 'target', 'dist', 'node_modules']
        
    def collect_files(self):
        """Collect all Rust and TypeScript source files in the source directory"""
        # Find all the files currently included in the concatenated file, including those added manually
        included_files = self._currently_included_files()

        # Clear existing lists
        self.rust_files = []
        self.ts_files = []
        self.other_files = []
        
        # Start with key Rust files
        self._add_if_exists('lib.rs', self.rust_files)
        self._add_if_exists('src/lib.rs', self.rust_files)
        self._add_if_exists('src/core/mod.rs', self.rust_files)
        
        # Start with key TypeScript files
        self._add_if_exists('ts/src/index.ts', self.ts_files)
        
        # Find all Rust and TypeScript files in the source directory
        for root, dirs, files in os.walk(self.source_dir):
            # Skip excluded directories
            dirs[:] = [d for d in dirs if d not in self.excluded_dirs]
            
            for file in files:
                full_path = os.path.join(root, file)
                rel_path = os.path.relpath(full_path, self.source_dir)
                
                # Collect Rust files
                if file.endswith('.rs'):
                    if rel_path not in self.rust_files:
                        self.rust_files.append(rel_path)
                
                # Collect TypeScript files
                elif file.endswith('.ts') and not file.endswith('.d.ts'):
                    if rel_path not in self.ts_files:
                        self.ts_files.append(rel_path)
        
        for rel_path in included_files:
            if rel_path not in self.rust_files and rel_path not in self.ts_files:
                if rel_path.endswith('.rs'):
                    self._add_if_exists(rel_path, self.rust_files)
                elif rel_path.endswith('.ts'):
                    self._add_if_exists(rel_path, self.ts_files)
                else:
                    self._add_if_exists(rel_path, self.other_files)
        
        print(f"Found {len(self.rust_files)} Rust files, {len(self.ts_files)} TypeScript files and {len(self.other_files)} other files.")
        return self.rust_files, self.ts_files, self.other_files
        
    def _add_if_exists(self, file_path, file_list):
        """Add a file to the list if it exists"""
        full_path = os.path.join(self.source_dir, file_path)
        if os.path.exists(full_path):
            file_list.append(file_path)
            return True
        return False
        
    def generate_concatenated_file(self):
        """Generate a new concatenated file from all source files"""
        self.collect_files()
        
        with open(self.output_file, 'w') as output:
            output.write("=====================================================================\n")
            output.write("= SORADYNE PROTOCOL - COMPLETE SOURCE CODE                         =\n")
            output.write(f"= Generated on: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}                           =\n")
            output.write(f"= Total files: {len(self.rust_files) + len(self.ts_files)}                                               =\n")
            output.write("=====================================================================\n\n")
            
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
            
            # Add TypeScript files
            output.write("\n\n\n=====================================================================\n")
            output.write("= TYPESCRIPT SOURCE CODE                                          =\n")
            output.write("=====================================================================\n\n")
            
            for file_path in self.ts_files:
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
            output.write("= OTHER FILES                                                  =\n")
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
            output.write("= END OF FILES                                                  =\n")
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
        ts_files = [f for f in included_files if f.endswith('.ts')]
        
        print("\nRust files:")
        for i, file_path in enumerate(rust_files):
            print(f"{i+1}. {file_path}")
        
        print("\nTypeScript files:")
        for i, file_path in enumerate(ts_files):
            print(f"{i+1}. {file_path}")

        print("\nOther files:")
        for i, file_path in enumerate(included_files):
            if not (file_path.endswith('.rs') or file_path.endswith('.ts')):
                print(f"{i+1}. {file_path}")
        
        print(f"\nTotal: {len(included_files)} files ({len(rust_files)} Rust, {len(ts_files)} TypeScript)")

def main():
    parser = argparse.ArgumentParser(description='Manage Soradyne source code concatenation')
    parser.add_argument('--source', default='.', help='Source directory containing Rust and TypeScript files')
    parser.add_argument('--output', default='soradyne_complete.txt', help='Output file path')
    
    subparsers = parser.add_subparsers(dest='command', help='Command to execute')
    
    generate_parser = subparsers.add_parser('generate', help='Generate a new concatenated file')
    
    add_parser = subparsers.add_parser('add', help='Add a new file to the existing concatenated file')
    add_parser.add_argument('file', help='Path to the file to add')
    
    list_parser = subparsers.add_parser('list', help='List all files included in the concatenation')
    
    interactive_parser = subparsers.add_parser('interactive', help='Run in interactive mode')
    
    args = parser.parse_args()
    
    manager = SoradyneManager(args.source, args.output)
    
    if args.command == 'generate':
        manager.generate_concatenated_file()
    elif args.command == 'add':
        manager.add_file(args.file)
    elif args.command == 'list':
        manager.list_files()
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
            print("  generate    - Generate a new concatenated file")
            print("  add <file>  - Add a new file to the concatenated file")
            print("  list        - List all files in the concatenation")
            print("  exit/quit   - Exit the program")
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
        elif cmd:
            print(f"Unknown command: '{cmd}'. Type 'help' for a list of commands.")

if __name__ == '__main__':
    main()