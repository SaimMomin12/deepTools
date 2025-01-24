import deeptools.plotFingerprint as pf

import os.path
from os import unlink
from matplotlib.testing.compare import compare_images


ROOT = os.path.dirname(os.path.abspath(__file__)) + "/test_data/"
BAM_IN1 = ROOT + "bowtie2_test1.bam"
FIN_PLOT1 = ROOT + "plotFingerprint_result1.png"
FIN_PLOT2 = ROOT + "plotFingerprint_result2.png"


def test_fingerprint_plot_with_minimal_options():
    """
    Test minimal command line args for fingerprint plot
    """
    out_png = '/tmp/fingerprint_plot1.png'
    args = "-b {} {} -o {}".format(BAM_IN1, BAM_IN1, out_png).split()
    pf.main(args)

    res = compare_images(FIN_PLOT1, out_png, 50)
    assert res is None, "Plots do not match"
    unlink(out_png)


def test_fingerprint_plot_with_advance_options():
    """
    Test command line args for fingerprint plot with additional options
    """
    out_rawcounts = '/tmp/rawcounts.tsv'
    out_qualitymetric = '/tmp/qualitymetrics.tsv'
    out_png2 = '/tmp/fingerprint_plot2.png'
    args = "-b {0} {0} -o {1} --outRawCounts {2} --outQualityMetrics {3} -T Test_Fingerpring_Plot --JSDsample {0}".format(BAM_IN1, out_png2, out_rawcounts, out_qualitymetric).split()
    pf.main(args)

    _foo = open(out_rawcounts, "r")
    result = _foo.readlines()[2:7]
    _expected = ["1327\t1327\n", "2015\t2015\n", "2928\t2928\n", "4419\t4419\n","8644\t8644\n"]
    assert result == _expected, "Contens in rawcounts files differ"

    with open(out_qualitymetric, "r") as _foo:
        result2 = len(_foo.readlines())
    _expected = 3
    assert result2 == _expected, "No of lines in quality metrics files differ"

    res = compare_images(FIN_PLOT2, out_png2, 50)
    assert res is None, "Plots do not match"
    unlink(out_png2)
    unlink(out_rawcounts)
    unlink(out_qualitymetric)
