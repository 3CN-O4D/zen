import os

def read_file(path):
    path = os.path.expanduser(path)
    with open(path, 'r') as f:
        return f.read()

def resolve_path(path):
    return os.path.abspath(os.path.expanduser(path))
