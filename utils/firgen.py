#!/usr/bin/env python3

# import wave
# with wave.open("filter.wav", mode='wb') as w:
fs = 96000
from scipy.signal import firwin
import numpy as np
filter = firwin(512, 200, 500, fs=fs)
filter = filter * (1<<15)
filter = filter.astype(np.int16)

from scipy.io import wavfile

wavfile.write("filter.wav", fs, filter)
