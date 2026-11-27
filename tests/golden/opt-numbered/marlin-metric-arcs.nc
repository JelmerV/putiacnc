N10 (Summary, written by PutiaCNC)
N20 (Spindle (!):            Not run by the program: it stops -M0- to ask you to start it at the)
N30 (                        speed shown, and again before a tool change and at the end to stop it.)
N40 (                        There is no work coordinate system: jog to the zero of the job and set)
N50 (                        it with G92 X0 Y0 Z0 first.)
N60 (Tools:                  3 mm flat end mill, 6 mm flat end mill. No tool change.)
N70 (Pauses:                 The program stops 4 times with the spindle off: 'Start the spindle at)
N80 (                        12000 rpm, then resume', 'Stop the spindle and change tool to 6 mm,)
N90 (                        then touch off Z 0 and resume', 'Start the spindle at 10000 rpm, then)
N100 (                        resume', 'Stop the spindle, then resume'. Resume it when you are)
N110 (                        ready; the spindle starts again by itself.)
N120 (Cutting:                Down to Z -1 mm. 94 mm of cutting, about 11 s in all.)
N130 (End:                    Lifts to Z 5 mm, goes to X0 Y0.)
N140 ; PutiaCNC - Marlin (no spindle control)
N150 ; Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface
N160 G21 G90
N170 ; Marlin: jog the tool to the zero of the job and set it there with G92 X0 Y0 Z0 before you run this file
N180 ; Spindle: by hand. The program stops to ask you to start and stop it
N190 G0 Z5
N200 ; Tool: 3 mm flat end mill
N210 M0 Start the spindle at 12000 rpm, then resume
N220 ; Operation 1: outside, tool 3 mm
N230 G0 X20.344 Y3.54
N240 G0 Z0.5
N250 G1 Z0 F250
N260 ; Pass 1
N270 G1 Z-1
N280 G1 F800
N290 G3 X21.5 Y5 I-0.344 J1.46
N300 G1 Y15
N310 G3 X20 Y16.5 I-1.5 J0
N320 G1 X5
N330 G3 X3.5 Y15 I0 J-1.5
N340 G1 Y5
N350 G3 X5 Y3.5 I1.5 J0
N360 G1 X20
N370 G1 X20.344 Y3.54
N380 G0 Z5
N390 M0 Stop the spindle and change tool to 6 mm, then touch off Z 0 and resume
N400 ; Tool: 6 mm flat end mill
N410 M0 Start the spindle at 10000 rpm, then resume
N420 ; Operation 2: inside, tool 6 mm
N430 G0 X44.618 Y20.015
N440 G0 Z0.5
N450 G1 Z0 F300
N460 ; Pass 1
N470 G1 Z-1
N480 G1 F1000
N490 G2 X45.382 Y29.985 I0.382 J4.985
N500 G2 X45.116 Y20.001 I-0.382 J-4.985
N510 G1 X44.618 Y20.015
N520 G0 Z5
N530 M0 Stop the spindle, then resume
N540 G0 X0 Y0
N550 ; End of program