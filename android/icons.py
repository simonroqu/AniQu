# Wandelt qu-core-Icons (16-px-SVG, nur <path>) in Android-Vektorgrafiken um: python3 icons.py back home …
# Ergebnis: app/src/main/res/drawable/qu_<name>.xml. Farbe setzt Compose (Icon tint), daher hier Weiß.
import re, sys, pathlib
src = pathlib.Path(__file__).parent / "../../quCore/data/icons"
out = pathlib.Path(__file__).parent / "app/src/main/res/drawable"
out.mkdir(parents=True, exist_ok=True)
for name in sys.argv[1:]:
    svg = (src / f"qu-{name}-symbolic.svg").read_text()
    paths = []
    for p in re.findall(r"<path\b[^>]*>", svg):
        d = re.search(r'\bd="([^"]+)"', p).group(1)
        rule = ' android:fillType="evenOdd"' if 'evenodd' in p else ""
        paths.append(f'    <path android:fillColor="#FFFFFFFF"{rule} android:pathData="{d}" />')
    assert paths and "<circle" not in svg and "<rect" not in svg and "transform" not in svg, name
    (out / f"qu_{name}.xml").write_text(
        '<vector xmlns:android="http://schemas.android.com/apk/res/android" android:width="16dp" android:height="16dp"\n'
        '    android:viewportWidth="16" android:viewportHeight="16">\n' + "\n".join(paths) + "\n</vector>\n")
    print("qu_" + name)
