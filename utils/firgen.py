#!/usr/bin/env python3
# Generates a low-pass filter at 200hz with a 500hz transition period for smoke-test testing of FIR import
from scipy.signal import firwin
import numpy as np
from scipy.io import wavfile

fs = 96000
filter = firwin(512, 200, 500, fs=fs)
filter = filter * (1<<15)
filter = filter.astype(np.int16)
wavfile.write("filter.wav", fs, filter)
