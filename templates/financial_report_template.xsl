<!-- FILE: templates/financial_report_template.xsl -->
<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format">

    <!-- Page Layout -->
    <fo:simple-page-master page-width="8.5in" page-height="11in"
                           margin-top="50pt" margin-bottom="50pt"
                           margin-left="50pt" margin-right="50pt"/>

    <!-- ============================================= -->
    <!-- NAMED TEMPLATE for a standard financial row   -->
    <!-- ============================================= -->
    <xsl:template name="render-row-item">
        <row>
            <cell style="td"><text><xsl:value-of select="label"/></text></cell>
            <cell style="td-right td"><text>{{formatCurrency value}}</text></cell>
        </row>
    </xsl:template>

    <!-- ============================================= -->
    <!-- NAMED TEMPLATE for a subtotal row             -->
    <!-- ============================================= -->
    <xsl:template name="render-row-subtotal">
        <row>
            <cell style="td subtotal-row-cell"><text><xsl:value-of select="label"/></text></cell>
            <cell style="td-right subtotal-row-cell"><text>{{formatCurrency value}}</text></cell>
        </row>
    </xsl:template>

    <!-- ============================================= -->
    <!-- NAMED TEMPLATE for a total row                -->
    <!-- ============================================= -->
    <xsl:template name="render-row-total">
        <row>
            <cell style="td total-row-cell"><text><xsl:value-of select="label"/></text></cell>
            <cell style="td-right total-row-cell"><text>{{formatCurrency value}}</text></cell>
        </row>
    </xsl:template>


    <!-- ============================================= -->
    <!-- Main Template (Entry Point)                   -->
    <!-- ============================================= -->
    <xsl:template match="/">
        <page-sequence select=".">
            <text style="h1"><xsl:value-of select="companyName"/></text>
            <text style="h2"><xsl:value-of select="reportTitle"/></text>

            <xsl:for-each select="sections">
                <text style="section-title"><xsl:value-of select="title"/></text>

                <xsl:for-each select="tables">
                    <xsl:if test="title">
                        <text style="table-title"><xsl:value-of select="title"/></text>
                    </xsl:if>

                    <table>
                        <columns>
                            <column width="70%"/>
                            <column width="30%"/>
                        </columns>

                        <!-- Table Body Content -->
                        <xsl:for-each select="data">
                            <!--
                              *** CHANGE IS HERE ***
                              Instead of embedding the row logic, we now call the
                              appropriate named template based on the data type.
                            -->
                            <xsl:if test="type = 'item'">
                                <xsl:call-template name="render-row-item"/>
                            </xsl:if>
                            <xsl:if test="type = 'subtotal'">
                                <xsl:call-template name="render-row-subtotal"/>
                            </xsl:if>
                            <xsl:if test="type = 'total'">
                                <xsl:call-template name="render-row-total"/>
                            </xsl:if>
                        </xsl:for-each>
                    </table>
                </xsl:for-each>
            </xsl:for-each>
        </page-sequence>
    </xsl:template>


    <!-- ============================================= -->
    <!-- Style Definitions                             -->
    <!-- ============================================= -->
    <xsl:attribute-set name="h1">
        <xsl:attribute name="font-size">32pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#142850</xsl:attribute>
        <xsl:attribute name="margin-bottom">10pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="h2">
        <xsl:attribute name="font-size">24pt</xsl:attribute>
        <xsl:attribute name="color">#27496d</xsl:attribute>
        <xsl:attribute name="margin-bottom">40pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="section-title">
        <xsl:attribute name="font-size">18pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="color">#142850</xsl:attribute>
        <xsl:attribute name="border-bottom">1pt solid #142850</xsl:attribute>
        <xsl:attribute name="padding-bottom">4pt</xsl:attribute>
        <xsl:attribute name="margin-top">20pt</xsl:attribute>
        <xsl:attribute name="margin-bottom">15pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="table-title">
        <xsl:attribute name="font-size">14pt</xsl:attribute>
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="margin-bottom">10pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td">
        <xsl:attribute name="font-size">11pt</xsl:attribute>
        <xsl:attribute name="padding-top">6pt</xsl:attribute>
        <xsl:attribute name="padding-bottom">6pt</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="td-right">
        <xsl:attribute name="text-align">right</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="subtotal-row-cell">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="border-top">0.5pt solid #888888</xsl:attribute>
    </xsl:attribute-set>

    <xsl:attribute-set name="total-row-cell">
        <xsl:attribute name="font-weight">bold</xsl:attribute>
        <xsl:attribute name="border-top">1.5pt solid #142850</xsl:attribute>
    </xsl:attribute-set>

</xsl:stylesheet>