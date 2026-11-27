(Summary, written by PutiaCNC)
(Zero:                   XY0 = lower-left corner of stock, Z0 = top surface, in G54)
(Start:                  Starts the spindle at 10000 rpm and waits 2 s, then moves to X44.618)
(                        Y20.015.)
(Tools:                  6 mm flat end mill. No tool change.)
(Cutting:                Down to Z -1 mm. 33 mm of cutting, about 7 s in all.)
(End:                    Lifts to Z 5 mm, stops the spindle, goes to X0 Y0.)
(File 2 of 2: 6 mm flat end mill operation 2)
(Before you start: change to 6 mm flat end mill and touch off Z0 again.)
(PutiaCNC - Mach3 / LinuxCNC)
(Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface)
G21 G90 G17 G94
G40 G49 G80
G54
(start code: dust shoe down)
M8
G4 P1
G0 Z5
T1 M6 (Tool 1: 6 mm flat end mill)
M3 S10000
G4 P2
(Operation 2: inside, tool 6 mm)
G0 X44.618 Y20.015
G0 Z0.5
G1 Z0 F300
(Pass 1)
G1 Z-1
G1 F1000
G1 X44.122 Y20.078
G1 X43.645 Y20.187
G1 X43.153 Y20.354
G1 X42.714 Y20.553
G1 X42.282 Y20.803
G1 X41.877 Y21.095
G1 X41.492 Y21.437
G1 X41.146 Y21.815
G1 X40.875 Y22.174
G1 X40.604 Y22.617
G1 X40.377 Y23.095
G1 X40.222 Y23.528
G1 X40.101 Y24.005
G1 X40.001 Y24.884
G1 X40.015 Y25.382
G1 X40.078 Y25.878
G1 X40.187 Y26.355
G1 X40.354 Y26.847
G1 X40.553 Y27.286
G1 X40.803 Y27.718
G1 X41.095 Y28.123
G1 X41.437 Y28.508
G1 X41.782 Y28.826
G1 X42.174 Y29.125
G1 X42.617 Y29.396
G1 X43.079 Y29.616
G1 X43.528 Y29.778
G1 X44.022 Y29.903
G1 X44.884 Y29.999
G1 X45.382 Y29.985
G1 X45.878 Y29.922
G1 X46.355 Y29.813
G1 X46.847 Y29.646
G1 X47.286 Y29.447
G1 X47.718 Y29.197
G1 X48.123 Y28.905
G1 X48.508 Y28.563
G1 X48.854 Y28.185
G1 X49.125 Y27.826
G1 X49.396 Y27.383
G1 X49.623 Y26.905
G1 X49.778 Y26.472
G1 X49.903 Y25.978
G1 X49.999 Y25.116
G1 X49.984 Y24.601
G1 X49.919 Y24.107
G1 X49.813 Y23.645
G1 X49.642 Y23.142
G1 X49.447 Y22.714
G1 X49.185 Y22.264
G1 X48.905 Y21.877
G1 X48.563 Y21.492
G1 X48.196 Y21.156
G1 X47.826 Y20.875
G1 X47.383 Y20.604
G1 X46.921 Y20.384
G1 X46.472 Y20.222
G1 X45.978 Y20.097
G1 X45.116 Y20.001
G1 X44.618 Y20.015
G0 Z5
M5
G0 X0 Y0
M9
(end code: dust shoe up)
M30