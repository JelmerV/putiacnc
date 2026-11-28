N10 (Summary, written by PutiaCNC)
N20 (Zero:                   XY0 = lower-left corner of stock, Z0 = top surface, in G54)
N30 (Start:                  Starts the spindle at 12000 rpm and waits 2 s, then moves to X20.344)
N40 (                        Y3.54.)
N50 (Tools:                  3 mm flat end mill, 6 mm flat end mill. 1 tool change -M6-.)
N60 (Cutting:                Down to Z -1 mm. 94 mm of cutting, about 15 s in all.)
N70 (End:                    Lifts to Z 5 mm, stops the spindle, goes to X0 Y0.)
N80 (PutiaCNC - Mach3 / LinuxCNC)
N90 (Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface)
N100 G21 G90 G17 G94
N110 G40 G49 G80
N120 G54
N130 G0 Z5
N140 T1 M6 (Tool 1: 3 mm flat end mill)
N150 M3 S12000
N160 G4 P2
N170 (Operation 1: outside, tool 3 mm)
N180 G0 X20.344 Y3.54
N190 G0 Z0.5
N200 G1 Z0 F250
N210 (Pass 1)
N220 G1 Z-1
N230 G1 F800
N240 G3 X21.5 Y5 I-0.344 J1.46
N250 G1 Y15
N260 G3 X20 Y16.5 I-1.5 J0
N270 G1 X5
N280 G3 X3.5 Y15 I0 J-1.5
N290 G1 Y5
N300 G3 X5 Y3.5 I1.5 J0
N310 G1 X20
N320 G1 X20.344 Y3.54
N330 G0 Z5
N340 M5
N350 T2 M6 (Tool 2: 6 mm flat end mill)
N360 M3 S10000
N370 G4 P2
N380 (Operation 2: inside, tool 6 mm)
N390 G0 X44.618 Y20.015
N400 G0 Z0.5
N410 G1 Z0 F300
N420 (Pass 1)
N430 G1 Z-1
N440 G1 F1000
N450 G2 X45.382 Y29.985 I0.382 J4.985
N460 G2 X45.116 Y20.001 I-0.382 J-4.985
N470 G1 X44.618 Y20.015
N480 G0 Z5
N490 M5
N500 G0 X0 Y0
N510 M30