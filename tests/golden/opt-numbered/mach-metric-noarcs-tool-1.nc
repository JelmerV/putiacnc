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
N250 G1 X20.669 Y3.657
N260 G1 X20.959 Y3.846
N270 G1 X21.197 Y4.096
N280 G1 X21.372 Y4.395
N290 G1 X21.475 Y4.725
N300 G1 X21.5 Y5
N310 G1 Y15
N320 G1 X21.46 Y15.344
N330 G1 X21.343 Y15.669
N340 G1 X21.154 Y15.959
N350 G1 X20.904 Y16.197
N360 G1 X20.605 Y16.372
N370 G1 X20.275 Y16.475
N380 G1 X20 Y16.5
N390 G1 X5
N400 G1 X4.656 Y16.46
N410 G1 X4.331 Y16.343
N420 G1 X4.041 Y16.154
N430 G1 X3.803 Y15.904
N440 G1 X3.628 Y15.605
N450 G1 X3.525 Y15.275
N460 G1 X3.5 Y15
N470 G1 Y5
N480 G1 X3.54 Y4.656
N490 G1 X3.657 Y4.331
N500 G1 X3.846 Y4.041
N510 G1 X4.096 Y3.803
N520 G1 X4.395 Y3.628
N530 G1 X4.725 Y3.525
N540 G1 X5 Y3.5
N550 G1 X20
N560 G1 X20.344 Y3.54
N570 G0 Z5
N580 M5
N590 G0 X0 Y0
N600 M30