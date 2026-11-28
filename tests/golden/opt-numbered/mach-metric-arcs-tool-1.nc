N10 (Summary, written by PutiaCNC)
N20 (Zero:                   XY0 = lower-left corner of stock, Z0 = top surface, in G54)
N30 (Start:                  Starts the spindle at 12000 rpm and waits 2 s, then moves to X20.344)
N40 (                        Y3.54.)
N50 (Tools:                  3 mm flat end mill. No tool change.)
N60 (Cutting:                Down to Z -1 mm. 61 mm of cutting, about 9 s in all.)
N70 (End:                    Lifts to Z 5 mm, stops the spindle, goes to X0 Y0.)
N80 (File 1 of 2: 3 mm flat end mill operation 1)
N90 (PutiaCNC - Mach3 / LinuxCNC)
N100 (Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface)
N110 G21 G90 G17 G94
N120 G40 G49 G80
N130 G54
N140 G0 Z5
N150 T1 M6 (Tool 1: 3 mm flat end mill)
N160 M3 S12000
N170 G4 P2
N180 (Operation 1: outside, tool 3 mm)
N190 G0 X20.344 Y3.54
N200 G0 Z0.5
N210 G1 Z0 F250
N220 (Pass 1)
N230 G1 Z-1
N240 G1 F800
N250 G3 X21.5 Y5 I-0.344 J1.46
N260 G1 Y15
N270 G3 X20 Y16.5 I-1.5 J0
N280 G1 X5
N290 G3 X3.5 Y15 I0 J-1.5
N300 G1 Y5
N310 G3 X5 Y3.5 I1.5 J0
N320 G1 X20
N330 G1 X20.344 Y3.54
N340 G0 Z5
N350 M5
N360 G0 X0 Y0
N370 M30