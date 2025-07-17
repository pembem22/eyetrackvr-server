#!/usr/bin/env python3
import sys
from pathlib import Path
import xml.etree.ElementTree as ET

ANDROID_NS = "http://schemas.android.com/apk/res/android"


def android_attr(name):
    return f"{{{ANDROID_NS}}}{name}"


def load_tree(path):
    tree = ET.parse(path)
    root = tree.getroot()
    return tree, root


def attribs_equal(a, b):
    return dict(a) == dict(b)


def find_merge_target(base_root, patch_elem):
    """
    Finds a matching tag in base_root that has the same tag name
    and optionally a matching key attribute (like android:name).
    """
    for base_elem in base_root.findall(patch_elem.tag):
        # If tags are the same and share the same android:name (if present), consider mergeable
        name_attr = android_attr("name")
        if patch_elem.get(name_attr) and patch_elem.get(name_attr) == base_elem.get(
            name_attr
        ):
            return base_elem
        elif not patch_elem.get(name_attr) and base_elem.get(name_attr) is None:
            # Only one element of this tag type with no name attr (e.g., <application>)
            return base_elem
    return None


def merge_attributes(target, patch_elem):
    for attr, value in patch_elem.attrib.items():
        if target.get(attr) != value:
            target.set(attr, value)


def merge_elements(base_root, patch_root):
    for patch_elem in patch_root:
        target = find_merge_target(base_root, patch_elem)
        if target is not None:
            merge_attributes(target, patch_elem)
        else:
            # Avoid exact duplicates (e.g., duplicate permission)
            if not any(
                ET.tostring(patch_elem) == ET.tostring(existing)
                for existing in base_root.findall(patch_elem.tag)
            ):
                base_root.append(patch_elem)


def main():
    if len(sys.argv) != 3:
        print("Usage: merge_manifest.py <base_manifest> <patch_manifest>")
        sys.exit(1)

    base_path = Path(sys.argv[1])
    patch_path = Path(sys.argv[2])

    ET.register_namespace("android", ANDROID_NS)

    base_tree, base_root = load_tree(base_path)
    patch_tree, patch_root = load_tree(patch_path)

    merge_elements(base_root, patch_root)
    base_tree.write(base_path, encoding="utf-8", xml_declaration=True)
    print(f"Merged {patch_path} into {base_path}")


if __name__ == "__main__":
    main()
