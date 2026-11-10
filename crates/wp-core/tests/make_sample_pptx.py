"""Makes the sample PowerPoint used by the import tests (python-pptx)."""
import sys
from pptx import Presentation
from pptx.util import Inches
from PIL import Image

out = sys.argv[1] if len(sys.argv) > 1 else "sample.pptx"
p = Presentation()
p.slide_width, p.slide_height = Inches(13.333), Inches(7.5)
s = p.slides.add_slide(p.slide_layouts[0])
s.shapes.title.text = "Harvest Thanksgiving"
s.placeholders[1].text = "Sunday 15 November 2026"
s = p.slides.add_slide(p.slide_layouts[1])
s.shapes.title.text = "Give by Mobile Money"
s.placeholders[1].text = "MTN MoMo: 024 555 1234\nName: Grace Chapel"
s = p.slides.add_slide(p.slide_layouts[5])
s.shapes.title.text = "Ayɛyi"
tb = s.shapes.add_textbox(Inches(1), Inches(2), Inches(6), Inches(1))
tb.text_frame.text = "Ɔdomankoma"
Image.new("RGB", (64, 64), (200, 80, 40)).save("/tmp/pic.png")
s.shapes.add_picture("/tmp/pic.png", Inches(8), Inches(2))
s = p.slides.add_slide(p.slide_layouts[1])
s.shapes.title.text = "Hidden slide"
s._element.set("show", "0")
p.save(out)
print(out)
