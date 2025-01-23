import deeptools.plotProfile as pp

import os.path
from os import unlink
from matplotlib.testing.compare import compare_images

ROOT = os.path.dirname(os.path.abspath(__file__)) + "/test_data/"
MATRIX_IN = ROOT + "computeMatrix_result1.gz"
PNG_OUT1 = ROOT + "profiler_result1.png"
PNG_OUT2 = ROOT + "profiler_result2.png"


def test_profiler_plot_with_minimal_args():
    """
    Test minimal command line args for profiler plot
    """
    outfile1 = '/tmp/profiler1.png'
    args = "-m {} -o {}".format(MATRIX_IN, outfile1).split()
    pp.main(args)

    res = compare_images(PNG_OUT1, outfile1, 50)
    assert res is None, res
    unlink(outfile1)


def test_profiler_plot_with_advance_args():
    """
    Test advance command line args for profiler plot
    """
    outfile2 = '/tmp/profiler2.png'
    profile_out = '/tmp/profiler_res.tsv'
    args = "-m {} -o {} --outFileNameData {}".format(MATRIX_IN, outfile2, profile_out).split()
    pp.main(args)

    res = compare_images(PNG_OUT2, outfile2, 50)
    assert res is None, res
    unlink(outfile2)

    _foo = open(profile_out, "r")
    resp = _foo.readlines()[2]
    _foo.close()
    expected = 'bamCoverage_result4_bw_0\tgenes\t2477942.875\t2610260.125\n'
    assert expected in resp, f"'{expected}' not found in '{resp}'"
    unlink(profile_out)
