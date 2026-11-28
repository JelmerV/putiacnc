N10 (Summary, written by PutiaCNC)
N20 (Zero:                   XY0 = lower-left corner of stock, Z0 = top surface, in G54)
N30 (Start:                  Starts the spindle at 12000 rpm and waits 2 s, then moves to X20.344)
N40 (                        Y3.54.)
N50 (Tools:                  3 mm flat end mill, 6 mm flat end mill. The program pauses 1 time for)
N60 (                        you to change the tool -to 6 mm-.)
N70 (At a tool change (!):   Touch off Z0 again with the new tool before you resume: the controller)
N80 (                        does not know how long each tool is.)
N90 (Cutting:                Down to Z -1 mm. 94 mm of cutting, about 15 s in all.)
N100 (End:                    Lifts to Z 5 mm, stops the spindle, goes to X0 Y0.)
N110 (PutiaCNC - GRBL)
N120 (Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface)
N130 G21 G90 G17 G94 G54
N140 G0 Z5
N150 (Tool: 3 mm flat end mill)
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
N360 M0 ; Change tool to 6 mm, then touch off Z0 and resume
N370 (Tool: 6 mm flat end mill)
N380 M3 S10000
N390 G4 P2
N400 (Operation 2: inside, tool 6 mm)
N410 G0 X44.618 Y20.015
N420 G0 Z0.5
N430 G1 Z0 F300
N440 (Pass 1)
N450 G1 Z-1
N460 G1 F1000
N470 G2 X45.382 Y29.985 I0.382 J4.985
N480 G2 X45.116 Y20.001 I-0.382 J-4.985
N490 G1 X44.618 Y20.015
N500 G0 Z5
N510 M5
N520 G0 X0 Y0
N530 M2