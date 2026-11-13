"""Regression checks for release metadata updates. Uses temporary formulae only."""
import pathlib
import shutil
import shlex
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
HASH = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'

class Updaters(unittest.TestCase):
    def test_validation_precedes_writes(self):
        shells = [(shutil.which('pwsh'), 'ps1'), (shutil.which('bash'), 'sh')]
        if pathlib.Path('D:/Git/bin/bash.exe').exists():
            shells[1] = ('D:/Git/bin/bash.exe', 'sh')
        self.assertTrue(any(shell for shell, _ in shells), 'No supported shell available')
        for shell, suffix in shells:
            if not shell:
                continue
            for version, cask_hash, formula_hash, accepted in [
                ('0.1.4', 'bad', HASH, False),
                ('0.1.4', '0' * 64, HASH, False),
                ('0.1.4', HASH, 'bad', False),
                ('0.1.4', '', HASH, False),
                ('0.1.4', HASH, '', False),
                ('0.1.4/invalid', HASH, HASH, False),
                ('0.1.4\n', HASH, HASH, False),
                ('0.1.4', HASH + '\n', HASH, False),
                ('0.1.4', HASH, HASH + '\n', False),
                ('0.1.4', HASH, HASH, True),
            ]:
                with self.subTest(shell=suffix, version=version, cask=cask_hash, formula=formula_hash):
                    with tempfile.TemporaryDirectory() as directory:
                        folder = pathlib.Path(directory)
                        cask, formula = folder / 'cask.rb', folder / 'formula.rb'
                        shutil.copyfile(ROOT / 'Casks/clickless.rb', cask)
                        shutil.copyfile(ROOT / 'Formula/clickless.rb', formula)
                        before = cask.read_bytes(), formula.read_bytes()
                        names = ['version', 'sha256', 'formula-sha256', 'cask', 'formula', 'zip']
                        prefix = '--'
                        command = [shell]
                        if suffix == 'ps1':
                            names = ['Version', 'Sha256', 'FormulaSha256', 'CaskPath', 'FormulaPath', 'ZipPath']
                            prefix = '-'
                            command += ['-NoProfile', '-File']
                        command += [(ROOT / f'scripts/update-homebrew.{suffix}').as_posix()]
                        for name, value in zip(names, [version, cask_hash, formula_hash, cask.as_posix(), formula.as_posix(), (folder / 'absent.zip').as_posix()]):
                            if value:
                                command += [prefix + name, value]
                        if suffix == 'sh':
                            command = [shell, '-c', shlex.join(command)]
                        result = subprocess.run(command, capture_output=True, text=True, timeout=30)
                        if accepted:
                            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                            self.assertIn('version "0.1.4"', cask.read_text())
                            self.assertIn('/v0.1.4.tar.gz', formula.read_text())
                            for file in (cask, formula):
                                self.assertIn(f'sha256 "{HASH}"', file.read_text())
                                self.assertNotIn('disable!', file.read_text())
                        else:
                            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                            self.assertEqual((cask.read_bytes(), formula.read_bytes()), before)

if __name__ == '__main__':
    unittest.main()
