N10 (Summary, written by PutiaCNC)
N20 (Spindle (!):            Not run by the program: it stops -M0- to ask you to start it at the)
N30 (                        speed shown, and again before a tool change and at the end to stop it.)
N40 (                        There is no work coordinate system: jog to the zero of the job and set)
N50 (                        it with G92 X0 Y0 Z0 first.)
N60 (Tools:                  3 mm flat end mill. No tool change.)
N70 (Pauses:                 The program stops 2 times with the spindle off: 'Start the spindle at)
N80 (                        12000 rpm, then resume', 'Stop the spindle, then resume'. Resume it)
N90 (                        when you are ready; the spindle starts again by itself.)
N100 (Cutting:                Down to Z -1 mm. 61 mm of cutting, about 7 s in all.)
N110 (End:                    Lifts to Z 5 mm, goes to X0 Y0.)
N120 (File 1 of 2: 3 mm flat end mill operation 1)
N130 ; PutiaCNC - Marlin (no spindle control)
N140 ; Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface
N150 G21 G90
N160 ; Marlin: jog the tool to the zero of the job and set it there with G92 X0 Y0 Z0 before you run this file
N170 ; Spindle: by hand. The program stops to ask you to start and stop it
N180 G0 Z5
N190 ; Tool: 3 mm flat end mill
N200 M0 Start the spindle at 12000 rpm, then resume
N210 ; Operation 1: outside, tool 3 mm
N220 G0 X20.344 Y3.54
N230 G0 Z0.5
N240 G1 Z0 F250
N250 ; Pass 1
N260 G1 Z-1
N270 G1 F800
N280 G3 X21.5 Y5 I-0.344 J1.46
N290 G1 Y15
N300 G3 X20 Y16.5 I-1.5 J0
N310 G1 X5
N320 G3 X3.5 Y15 I0 J-1.5
N330 G1 Y5
N340 G3 X5 Y3.5 I1.5 J0
N350 G1 X20
N360 G1 X20.344 Y3.54
N370 G0 Z5
N380 M0 Stop the spindle, then resume
N390 G0 X0 Y0
N400 ; End of program