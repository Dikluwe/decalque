import importlib.util
import json
import pathlib
import tempfile
import unittest


ROOT = pathlib.Path(__file__).parents[1]
SPEC = importlib.util.spec_from_file_location("chapter_book_splitter", ROOT / "chapter_book_splitter.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ChapterBookSplitterTests(unittest.TestCase):
    def test_validates_exact_contiguous_coverage(self):
        manifest = {"schema_version": 1, "total_pages": 4, "chapters": [
            {"id": "a", "title": "A", "start": 1, "end": 2, "evidence": "heading"},
            {"id": "b", "title": "B", "start": 3, "end": 4, "evidence": "editorial"},
        ]}
        self.assertEqual(MODULE.validate_manifest(manifest), manifest["chapters"])

    def test_rejects_gap_overlap_and_missing_evidence(self):
        base = {"schema_version": 1, "total_pages": 4}
        for chapters, message in [
            ([{"id": "a", "title": "A", "start": 1, "end": 2, "evidence": "heading"},
              {"id": "b", "title": "B", "start": 4, "end": 4, "evidence": "heading"}], "contíguos"),
            ([{"id": "a", "title": "A", "start": 1, "end": 3, "evidence": "heading"},
              {"id": "b", "title": "B", "start": 3, "end": 4, "evidence": "heading"}], "contíguos"),
            ([{"id": "a", "title": "A", "start": 1, "end": 4}], "evidence"),
        ]:
            with self.subTest(message=message):
                with self.assertRaisesRegex(ValueError, message):
                    MODULE.validate_manifest({**base, "chapters": chapters})

    def test_assigns_review_pages_to_only_their_chapter(self):
        chapters = [
            {"id": "a", "title": "A", "start": 1, "end": 2, "evidence": "heading"},
            {"id": "b", "title": "B", "start": 3, "end": 4, "evidence": "heading"},
        ]
        report = MODULE.chapter_report(chapters[1], {2, 3, 4})
        self.assertEqual(report["review_pages"], [3, 4])
        self.assertEqual(report["page_count"], 2)


if __name__ == "__main__":
    unittest.main()
