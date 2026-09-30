#!/usr/bin/env python3
"""Package the local GPUI RSC syntax extension as a VSIX for Cursor."""

from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile


ROOT = Path(__file__).parent
OUTPUT = ROOT / "dist" / "gpui-rsc-syntax-0.1.0.vsix"
CONTENT_TYPES = '''<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="json" ContentType="application/json" />
  <Default Extension="vsixmanifest" ContentType="text/xml" />
  <Override PartName="/extension.vsixmanifest" ContentType="text/xml" />
  <Override PartName="/extension/package.json" ContentType="application/json" />
  <Override PartName="/extension/syntaxes/gpui-rsc.tmLanguage.json" ContentType="application/json" />
</Types>
'''
MANIFEST = '''<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">
  <Metadata>
    <Identity Language="en-US" Id="gpui-rsc-syntax" Version="0.1.0" Publisher="local" />
    <DisplayName>GPUI RSC Syntax</DisplayName>
    <Description>HTML and Rust syntax coloring for .rsc components.</Description>
    <Tags>rsc;rust;html</Tags>
    <Categories><Category>Programming Languages</Category></Categories>
    <Properties>
      <Property Id="Microsoft.VisualStudio.Code.Engine" Value=">=1.85.0" />
    </Properties>
  </Metadata>
  <Installation>
    <InstallationTarget Id="Microsoft.VisualStudio.Code" />
  </Installation>
  <Dependencies />
  <Assets>
    <Asset Type="Microsoft.VisualStudio.Code.Manifest" Path="extension/package.json" Addressable="true" />
    <Asset Type="Microsoft.VisualStudio.Code.TextMateGrammars" Path="extension/syntaxes/gpui-rsc.tmLanguage.json" Addressable="true" />
  </Assets>
</PackageManifest>
'''


def main() -> None:
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(OUTPUT, "w", ZIP_DEFLATED) as package:
        package.writestr("[Content_Types].xml", CONTENT_TYPES)
        package.writestr("extension.vsixmanifest", MANIFEST)
        package.write(ROOT / "package.json", "extension/package.json")
        package.write(
            ROOT / "syntaxes" / "gpui-rsc.tmLanguage.json",
            "extension/syntaxes/gpui-rsc.tmLanguage.json",
        )
    print(OUTPUT)


if __name__ == "__main__":
    main()
